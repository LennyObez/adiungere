//! The provenance commands: sign, timestamp, and the credentials half of verify.
//!
//! Each answers with text for a person or one JSON document for a pipeline, and a status: 0 when the
//! answer is a yes, 1 when it is a no, 2 when the question could not be asked. A "no" exists only for a
//! check: credentials whose signature or binding does not hold, or a token that does not name its file.
//! Nothing here returns a verdict about a recording: a signature says who put their name to some facts and
//! when, and the trust state says what the validator found about that name.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use adiungere_isobmff::FileSource;
use adiungere_manifest::wording::{Phrase, render};
use adiungere_manifest::{Manifest, Outcome, Scope, Subject};
use adiungere_provenance::{Algorithm, Credential, Kind, Placement, Request, State};

use crate::media::{MediaFailure, Output, Rendered};

/// How a signing is credentialled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credentialling {
    /// A self-signed chain generated for the run, on no trust list.
    Ephemeral,
    /// A chain and a key the operator holds, in PEM files.
    Held {
        /// The certificate chain, end entity first.
        chain: PathBuf,
        /// The key.
        key: PathBuf,
        /// The algorithm the key is for.
        algorithm: Algorithm,
    },
}

/// What a signing is done with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signing {
    /// The credential.
    pub credentialling: Credentialling,
    /// The time-stamping authority to ask, if any.
    pub time_authority: Option<String>,
}

impl Signing {
    /// The credential this describes, built now.
    ///
    /// # Errors
    ///
    /// Returns a failure when a PEM file cannot be read or the credential cannot be built from it.
    pub fn credential(&self) -> Result<Credential, MediaFailure> {
        let credential = match &self.credentialling {
            Credentialling::Ephemeral => Credential::ephemeral("adiungere.local"),
            Credentialling::Held {
                chain,
                key,
                algorithm,
            } => {
                let chain_pem = std::fs::read(chain).map_err(|cause| MediaFailure::Open {
                    path: chain.clone(),
                    cause: adiungere_isobmff::SourceError::Io(cause),
                })?;
                let key_pem = std::fs::read(key).map_err(|cause| MediaFailure::Open {
                    path: key.clone(),
                    cause: adiungere_isobmff::SourceError::Io(cause),
                })?;
                Credential::held(&chain_pem, &key_pem, *algorithm)
            },
        }
        .map_err(|cause| MediaFailure::Provenance {
            cause: cause.to_string(),
        })?;
        Ok(match &self.time_authority {
            Some(authority) => credential.stamped_by(authority),
            None => credential,
        })
    }
}

/// What a signing produced, as printed.
#[derive(Debug, serde::Serialize)]
struct SignedAnswer {
    placement: Placement,
    written: PathBuf,
    credential: Kind,
    time_authority: Option<String>,
    credentials: adiungere_provenance::Credentials,
}

/// The facts to sign for an asset: the manifest beside it when there is one for it, otherwise the
/// manifest of the file computed now.
fn facts_for(asset: &Path) -> Result<Manifest, MediaFailure> {
    let beside = crate::media::manifest_beside(asset);
    if beside.is_file() {
        let text = std::fs::read_to_string(&beside).map_err(|cause| MediaFailure::Manifest {
            path: beside.clone(),
            cause: cause.to_string(),
        })?;
        let manifest = Manifest::from_json(&text).map_err(|cause| MediaFailure::Manifest {
            path: beside.clone(),
            cause: cause.to_string(),
        })?;
        if manifest.file.name == crate::media::file_name(asset) {
            return Ok(manifest);
        }
    }
    let mut source = FileSource::open(asset).map_err(|cause| MediaFailure::Open {
        path: asset.to_path_buf(),
        cause,
    })?;
    let name = crate::media::file_name(asset);
    let file_name_time = adiungere_scan::parse_name(&name).and_then(|parsed| parsed.time);
    adiungere_manifest::build(
        &mut source,
        &name,
        Scope::Full,
        &crate::media::producer(),
        file_name_time,
    )
    .map_err(|cause| MediaFailure::Read {
        path: asset.to_path_buf(),
        cause,
    })
}

/// Signs an asset: embedded into a copy written at `out` when `embed` is set, beside the asset otherwise.
///
/// # Errors
///
/// Returns a failure when the asset or a source cannot be read, when the credential cannot be built,
/// when the signing fails, or when what was written cannot be read back.
pub fn sign(
    asset: &Path,
    sources: &[PathBuf],
    embed: bool,
    out: Option<&Path>,
    signing: &Signing,
    output: Output,
) -> Result<Rendered, MediaFailure> {
    let facts = facts_for(asset)?;
    let credential = signing.credential()?;
    let placement = if embed {
        Placement::Embedded
    } else {
        Placement::Sidecar
    };
    let request = Request {
        asset,
        sources: sources.iter().map(PathBuf::as_path).collect(),
        facts: &facts,
        placement,
    };
    let signed =
        adiungere_provenance::sign(&request, &credential, out).map_err(|cause| MediaFailure::Provenance {
            cause: cause.to_string(),
        })?;
    let read_from = match placement {
        Placement::Embedded => signed.written.clone(),
        Placement::Sidecar => asset.to_path_buf(),
    };
    let credentials = adiungere_provenance::read(&read_from, Some(&signed.written)).map_err(|cause| {
        MediaFailure::Provenance {
            cause: cause.to_string(),
        }
    })?;
    let answer = SignedAnswer {
        placement,
        written: signed.written,
        credential: credential.kind(),
        time_authority: signing.time_authority.clone(),
        credentials,
    };
    let text = match output {
        Output::Json => crate::media::json(&answer)?,
        Output::Text => describe_signing(&answer, asset),
    };
    Ok(Rendered {
        text,
        negative: false,
    })
}

fn signer_of(credentials: &adiungere_provenance::Credentials) -> (String, String) {
    let signer = credentials.signer.as_ref();
    (
        signer
            .and_then(|s| s.common_name.clone())
            .unwrap_or_else(|| "an unnamed signer".to_owned()),
        signer
            .and_then(|s| s.algorithm.clone())
            .unwrap_or_else(|| "unknown algorithm".to_owned()),
    )
}

fn describe_signing(answer: &SignedAnswer, asset: &Path) -> String {
    let mut text = String::new();
    let (signer, algorithm) = signer_of(&answer.credentials);
    let name = crate::media::file_name(asset);
    let line = match answer.placement {
        Placement::Embedded => render(
            Phrase::SignEmbedded,
            &[
                ("name", &crate::media::file_name(&answer.written)),
                ("signer", &signer),
                ("algorithm", &algorithm),
            ],
        ),
        Placement::Sidecar => render(
            Phrase::SignSidecar,
            &[
                ("name", &name),
                ("path", &answer.written.display().to_string()),
                ("signer", &signer),
                ("algorithm", &algorithm),
            ],
        ),
    };
    let _ = writeln!(text, "{line}");
    if answer.credential == Kind::Ephemeral {
        let _ = writeln!(text, "  {}", render(Phrase::SignEphemeral, &[]));
    }
    match &answer.time_authority {
        Some(authority) => {
            let _ = writeln!(
                text,
                "  {}",
                render(Phrase::SignTime, &[("authority", authority)])
            );
        },
        None => {
            let _ = writeln!(text, "  {}", render(Phrase::SignTimeNone, &[]));
        },
    }
    describe_credentials(&mut text, &answer.credentials, None);
    let _ = writeln!(text);
    let _ = writeln!(text, "{}", render(Phrase::FactsOnly, &[]));
    text
}

/// What a check of a file's credentials found, compared with the file when the signed facts read.
#[derive(Debug, serde::Serialize)]
pub struct CredentialsCheck {
    /// The credentials, as read.
    pub credentials: adiungere_provenance::Credentials,
    /// How many tracks of the signed facts do not carry the fingerprints the file has, when the facts
    /// read as an adiungere manifest.
    pub tracks_differing: Option<usize>,
}

/// Reads and checks a file's credentials, embedded or beside it, against the file.
///
/// # Errors
///
/// Returns a failure when the file cannot be read or the library refuses; a file with no credentials is
/// not a failure but an answer, `None`.
pub fn check_credentials(
    file: &Path,
    sidecar: Option<&Path>,
) -> Result<Option<CredentialsCheck>, MediaFailure> {
    let credentials = match adiungere_provenance::read(file, sidecar) {
        Ok(credentials) => credentials,
        Err(adiungere_provenance::Error::NoCredentials { .. }) => return Ok(None),
        Err(cause) => {
            return Err(MediaFailure::Provenance {
                cause: cause.to_string(),
            });
        },
    };
    let tracks_differing = match &credentials.integrity {
        Some(facts) => {
            let mut source = FileSource::open(file).map_err(|cause| MediaFailure::Open {
                path: file.to_path_buf(),
                cause,
            })?;
            let verification =
                adiungere_manifest::verify(facts, &mut source).map_err(|cause| MediaFailure::Read {
                    path: file.to_path_buf(),
                    cause,
                })?;
            Some(
                verification
                    .findings
                    .iter()
                    .filter(|finding| matches!(finding.subject, Subject::Track { .. }))
                    .filter(|finding| !matches!(finding.outcome, Outcome::Identical))
                    .count(),
            )
        },
        None => None,
    };
    Ok(Some(CredentialsCheck {
        credentials,
        tracks_differing,
    }))
}

/// Appends the description of credentials to a text: where they were, who signed, what holds.
pub fn describe_credentials(
    text: &mut String,
    credentials: &adiungere_provenance::Credentials,
    tracks_differing: Option<usize>,
) {
    let (signer, algorithm) = signer_of(credentials);
    let generator = credentials
        .generator
        .clone()
        .unwrap_or_else(|| "an unnamed generator".to_owned());
    let found = match credentials.placement {
        Placement::Embedded => render(
            Phrase::CredentialsEmbedded,
            &[
                ("signer", &signer),
                ("algorithm", &algorithm),
                ("generator", &generator),
            ],
        ),
        Placement::Sidecar => render(
            Phrase::CredentialsSidecar,
            &[
                ("path", &credentials.read_from.display().to_string()),
                ("signer", &signer),
                ("algorithm", &algorithm),
                ("generator", &generator),
            ],
        ),
    };
    let _ = writeln!(text, "  {found}");
    let state = match credentials.state {
        State::Trusted => render(Phrase::CredentialsOnList, &[]),
        State::ValidNotOnTrustList => render(Phrase::CredentialsNotOnList, &[]),
        State::Invalid => {
            let codes: Vec<&str> = credentials
                .statuses
                .iter()
                .filter(|status| !status.passed)
                .map(|status| status.code.as_str())
                .collect();
            render(Phrase::CredentialsInvalid, &[("codes", &codes.join(", "))])
        },
    };
    let _ = writeln!(text, "  {state}");
    match &credentials.time_stamped_at {
        Some(time) => {
            let _ = writeln!(text, "  {}", render(Phrase::CredentialsTime, &[("time", time)]));
        },
        None => {
            let _ = writeln!(text, "  {}", render(Phrase::CredentialsTimeNone, &[]));
        },
    }
    if !credentials.actions.is_empty() {
        let _ = writeln!(
            text,
            "  {}",
            render(
                Phrase::CredentialsActions,
                &[("list", &credentials.actions.join(", "))]
            )
        );
    }
    if !credentials.ingredients.is_empty() {
        let list: Vec<String> = credentials
            .ingredients
            .iter()
            .map(|ingredient| {
                format!(
                    "{} ({})",
                    ingredient.title.as_deref().unwrap_or("unnamed"),
                    ingredient.relationship
                )
            })
            .collect();
        let _ = writeln!(
            text,
            "  {}",
            render(Phrase::CredentialsSources, &[("list", &list.join(", "))])
        );
    }
    let facts = match (&credentials.integrity, tracks_differing) {
        (Some(facts), Some(0)) => render(Phrase::CredentialsFactsMatch, &[("name", &facts.file.name)]),
        (Some(facts), Some(count)) => render(
            Phrase::CredentialsFactsDiffer,
            &[("name", &facts.file.name), ("count", &count.to_string())],
        ),
        (Some(_), None) => String::new(),
        (None, _) => render(Phrase::CredentialsFactsNone, &[]),
    };
    if !facts.is_empty() {
        let _ = writeln!(text, "  {facts}");
    }
}

/// What a time-stamping produced, as printed.
#[derive(Debug, serde::Serialize)]
struct StampedAnswer {
    file: PathBuf,
    token: PathBuf,
    authority: String,
    time: String,
}

/// The token's default place: beside the file, with the suffix appended to the whole file name.
#[must_use]
pub fn token_beside(file: &Path) -> PathBuf {
    let mut name = crate::media::file_name(file);
    name.push_str(".tsr");
    file.with_file_name(name)
}

/// Asks an authority for a time-stamp token over a file and writes the token beside it.
///
/// # Errors
///
/// Returns a failure when the file cannot be read, when the authority does not answer with a token over
/// it, or when the token cannot be written.
pub fn timestamp(
    file: &Path,
    authority: &str,
    out: Option<&Path>,
    output: Output,
) -> Result<Rendered, MediaFailure> {
    let bytes = std::fs::read(file).map_err(|cause| MediaFailure::Open {
        path: file.to_path_buf(),
        cause: adiungere_isobmff::SourceError::Io(cause),
    })?;
    let token = adiungere_provenance::stamp(&bytes, authority).map_err(|cause| MediaFailure::Provenance {
        cause: cause.to_string(),
    })?;
    let path = out.map_or_else(|| token_beside(file), Path::to_path_buf);
    std::fs::write(&path, &token.bytes).map_err(|cause| MediaFailure::Write {
        path: path.clone(),
        cause,
    })?;
    let answer = StampedAnswer {
        file: file.to_path_buf(),
        token: path,
        authority: token.authority,
        time: token.time,
    };
    let text = match output {
        Output::Json => crate::media::json(&answer)?,
        Output::Text => format!(
            "{}\n\n{}\n",
            render(
                Phrase::StampWritten,
                &[
                    ("name", &crate::media::file_name(&answer.file)),
                    ("authority", &answer.authority),
                    ("path", &answer.token.display().to_string()),
                    ("time", &answer.time),
                ]
            ),
            render(Phrase::FactsOnly, &[])
        ),
    };
    Ok(Rendered {
        text,
        negative: false,
    })
}

/// What a token beside a manifest says about it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TokenCheck {
    /// The token's path.
    pub token: PathBuf,
    /// The time attested, when the token names the manifest.
    pub time: Option<String>,
    /// Why the token does not name the manifest, when it does not.
    pub mismatch: Option<String>,
}

/// Checks the token beside a manifest, when there is one.
///
/// # Errors
///
/// Returns a failure when the manifest or the token cannot be read.
pub fn check_token(manifest_path: &Path) -> Result<Option<TokenCheck>, MediaFailure> {
    let token_path = token_beside(manifest_path);
    if !token_path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(manifest_path).map_err(|cause| MediaFailure::Manifest {
        path: manifest_path.to_path_buf(),
        cause: cause.to_string(),
    })?;
    let token = std::fs::read(&token_path).map_err(|cause| MediaFailure::Manifest {
        path: token_path.clone(),
        cause: cause.to_string(),
    })?;
    Ok(Some(match adiungere_provenance::check(&token, &bytes) {
        Ok(attested) => TokenCheck {
            token: token_path,
            time: Some(attested.time),
            mismatch: None,
        },
        Err(cause) => TokenCheck {
            token: token_path,
            time: None,
            mismatch: Some(cause.to_string()),
        },
    }))
}

/// Appends the description of a token check to a text.
pub fn describe_token(text: &mut String, check: &TokenCheck) {
    let path = check.token.display().to_string();
    let line = match (&check.time, &check.mismatch) {
        (Some(time), _) => render(Phrase::StampToken, &[("path", &path), ("time", time)]),
        (None, Some(reason)) => render(Phrase::StampTokenMismatch, &[("path", &path), ("reason", reason)]),
        (None, None) => String::new(),
    };
    if !line.is_empty() {
        let _ = writeln!(text, "  {line}");
    }
}
