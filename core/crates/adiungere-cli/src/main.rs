//! The adiungere command line.
//!
//! Today it reads the evidence register. Everything else in the roadmap arrives with the milestone that
//! gives it something to do, because a subcommand that prints a placeholder is worse than a subcommand that
//! does not exist.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use adiungere_cli::evidence::{ProbeId, Register, Verdict};
use adiungere_cli::repository;
use adiungere_cli::roadmap::{MilestoneId, Roadmap};
use clap::{Args, Parser, Subcommand, ValueEnum};

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(failure) => {
            eprintln!("adiungere: {failure}");
            ExitCode::from(2)
        },
    }
}

/// What a person asked the command line to do.
#[derive(Debug, Parser)]
#[command(
    name = "adiungere",
    version,
    about = "Find, play, export and check multi-stream dashcam recordings.",
    long_about = "adiungere reads dashcam recordings that hold more than one camera in a single file.\n\n\
                  At this milestone the command line reads the project's evidence register, which records \
                  every load-bearing assumption in the project together with its verdict."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Read the evidence register.
    Probes(Probes),
}

#[derive(Debug, Args)]
struct Probes {
    #[command(subcommand)]
    action: ProbeAction,

    /// Read the register from this file instead of finding it in the repository.
    #[arg(long, value_name = "PATH", global = true)]
    register: Option<PathBuf>,

    /// Read the roadmap from this file instead of finding it in the repository.
    #[arg(long, value_name = "PATH", global = true)]
    roadmap: Option<PathBuf>,

    /// How to print the answer.
    #[arg(long, value_enum, default_value_t = Format::Text, global = true)]
    format: Format,
}

#[derive(Debug, Subcommand)]
enum ProbeAction {
    /// List the probes, most recent identifier last.
    List {
        /// Only probes carrying this verdict.
        #[arg(long, value_name = "VERDICT")]
        verdict: Option<String>,

        /// Only probes assigned to this milestone.
        #[arg(long, value_name = "MILESTONE")]
        milestone: Option<String>,
    },

    /// Show one probe in full.
    Show {
        /// The identifier, such as P11.
        #[arg(value_name = "IDENTIFIER")]
        id: String,
    },

    /// Check that the register and the roadmap agree.
    Check,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    /// Aligned columns, for a person.
    Text,
    /// One JSON document, for a pipeline.
    Json,
}

/// Anything that stops the command line before it can answer.
#[derive(Debug)]
enum Failure {
    NoRepository,
    Unreadable {
        path: PathBuf,
        cause: std::io::Error,
    },
    Register {
        path: PathBuf,
        cause: adiungere_cli::evidence::ParseError,
    },
    Roadmap {
        path: PathBuf,
        cause: adiungere_cli::roadmap::ParseError,
    },
    UnknownVerdict {
        value: String,
    },
    UnknownMilestone {
        value: String,
    },
    UnknownIdentifier {
        value: String,
    },
    NoSuchProbe {
        id: ProbeId,
    },
    Serialisation {
        cause: serde_json::Error,
    },
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRepository => formatter.write_str(
                "no repository found above the working directory. Run this inside a checkout, or pass \
                 --register and --roadmap",
            ),
            Self::Unreadable { path, cause } => {
                write!(formatter, "cannot read {}: {cause}", path.display())
            },
            Self::Register { path, cause } => {
                write!(formatter, "{}: {cause}", path.display())
            },
            Self::Roadmap { path, cause } => {
                write!(formatter, "{}: {cause}", path.display())
            },
            Self::UnknownVerdict { value } => write!(
                formatter,
                "\"{value}\" is not a verdict; use measured, reasoned, unavailable or not started"
            ),
            Self::UnknownMilestone { value } => {
                write!(formatter, "\"{value}\" is not a milestone identifier such as M1")
            },
            Self::UnknownIdentifier { value } => {
                write!(formatter, "\"{value}\" is not a probe identifier such as P11")
            },
            Self::NoSuchProbe { id } => write!(formatter, "the register holds no {id}"),
            Self::Serialisation { cause } => write!(formatter, "cannot write the answer: {cause}"),
        }
    }
}

fn run() -> Result<ExitCode, Failure> {
    let cli = Cli::parse();

    match cli.command {
        Command::Probes(probes) => run_probes(probes),
    }
}

fn run_probes(probes: Probes) -> Result<ExitCode, Failure> {
    let (register_path, roadmap_path) = resolve_paths(probes.register, probes.roadmap)?;
    let register = read_register(&register_path)?;

    match probes.action {
        ProbeAction::List { verdict, milestone } => {
            let verdict = match verdict {
                None => None,
                Some(value) => Some(
                    value
                        .parse::<Verdict>()
                        .map_err(|()| Failure::UnknownVerdict { value: value.clone() })?,
                ),
            };

            let milestone = match milestone {
                None => None,
                Some(value) => Some(
                    value
                        .parse::<MilestoneId>()
                        .map_err(|()| Failure::UnknownMilestone { value: value.clone() })?,
                ),
            };

            list(&register, verdict, milestone, probes.format)
        },

        ProbeAction::Show { id } => {
            let id = id
                .parse::<ProbeId>()
                .map_err(|()| Failure::UnknownIdentifier { value: id.clone() })?;
            let probe = register.get(id).ok_or(Failure::NoSuchProbe { id })?;

            show(probe, probes.format)
        },

        ProbeAction::Check => {
            let roadmap = read_roadmap(&roadmap_path)?;
            check(&register, &roadmap, probes.format)
        },
    }
}

fn resolve_paths(register: Option<PathBuf>, roadmap: Option<PathBuf>) -> Result<(PathBuf, PathBuf), Failure> {
    if let (Some(register), Some(roadmap)) = (register.as_ref(), roadmap.as_ref()) {
        return Ok((register.clone(), roadmap.clone()));
    }

    let here = std::env::current_dir().map_err(|cause| Failure::Unreadable {
        path: PathBuf::from("."),
        cause,
    })?;
    let root = repository::locate_root(&here).ok_or(Failure::NoRepository)?;

    Ok((
        register.unwrap_or_else(|| root.join(repository::REGISTER)),
        roadmap.unwrap_or_else(|| root.join(repository::ROADMAP)),
    ))
}

fn read_register(path: &Path) -> Result<Register, Failure> {
    let source = std::fs::read_to_string(path).map_err(|cause| Failure::Unreadable {
        path: path.to_path_buf(),
        cause,
    })?;

    Register::parse(&source).map_err(|cause| Failure::Register {
        path: path.to_path_buf(),
        cause,
    })
}

fn read_roadmap(path: &Path) -> Result<Roadmap, Failure> {
    let source = std::fs::read_to_string(path).map_err(|cause| Failure::Unreadable {
        path: path.to_path_buf(),
        cause,
    })?;

    Roadmap::parse(&source).map_err(|cause| Failure::Roadmap {
        path: path.to_path_buf(),
        cause,
    })
}

fn list(
    register: &Register,
    verdict: Option<Verdict>,
    milestone: Option<MilestoneId>,
    format: Format,
) -> Result<ExitCode, Failure> {
    let selected: Vec<_> = register
        .probes()
        .iter()
        .filter(|probe| verdict.is_none_or(|wanted| probe.verdict == wanted))
        .filter(|probe| milestone.is_none_or(|wanted| probe.milestone == wanted))
        .collect();

    if format == Format::Json {
        let rendered =
            serde_json::to_string_pretty(&selected).map_err(|cause| Failure::Serialisation { cause })?;
        println!("{rendered}");
        return Ok(ExitCode::SUCCESS);
    }

    let widest = selected
        .iter()
        .map(|probe| probe.verdict.as_str().len())
        .max()
        .unwrap_or(0);

    for probe in &selected {
        println!(
            "{id}  {verdict:width$}  {milestone:>3}  {title}",
            id = probe.id,
            verdict = probe.verdict.as_str(),
            milestone = probe.milestone.to_string(),
            title = probe.title,
            width = widest,
        );
    }

    println!();
    println!("{}", summary(register, selected.len()));

    Ok(ExitCode::SUCCESS)
}

fn summary(register: &Register, shown: usize) -> String {
    let total = register.probes().len();
    let mut line = if shown == total {
        format!("{total} probes: ")
    } else {
        format!("{shown} of {total} probes shown. Across the whole register: ")
    };

    let counts = register.tally();

    for (index, (verdict, count)) in counts.iter().enumerate() {
        if index > 0 {
            line.push_str(", ");
        }

        let _ = write!(line, "{count} {verdict}");
    }

    line.push('.');
    line
}

fn show(probe: &adiungere_cli::evidence::Probe, format: Format) -> Result<ExitCode, Failure> {
    if format == Format::Json {
        let rendered =
            serde_json::to_string_pretty(probe).map_err(|cause| Failure::Serialisation { cause })?;
        println!("{rendered}");
        return Ok(ExitCode::SUCCESS);
    }

    println!("{} {}", probe.id, probe.title);
    println!();
    println!("  Verdict    {}", probe.verdict);
    println!("  Milestone  {}", probe.milestone);
    println!();
    print_paragraph("Question", &probe.question);
    print_paragraph("Method", &probe.method);
    print_paragraph("Decides", &probe.decides);

    match &probe.result {
        Some(result) => print_paragraph("Result", result),
        None => println!(
            "No result is recorded, because the verdict is \"{}\". That is not evidence of anything.",
            probe.verdict
        ),
    }

    Ok(ExitCode::SUCCESS)
}

/// How wide a paragraph is allowed to be when printed for a person.
const WRAP_AT: usize = 96;

fn print_paragraph(heading: &str, body: &str) {
    println!("{heading}");

    for line in body.lines() {
        if line.trim().is_empty() {
            println!();
            continue;
        }

        for wrapped in wrap(line, WRAP_AT) {
            println!("  {wrapped}");
        }
    }

    println!();
}

/// Breaks a line at word boundaries, leaving a word longer than the width on its own line.
fn wrap(line: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in line.split_whitespace() {
        if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut current));
        }

        if !current.is_empty() {
            current.push(' ');
        }

        current.push_str(word);
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn check(register: &Register, roadmap: &Roadmap, format: Format) -> Result<ExitCode, Failure> {
    let discrepancies = register.reconcile(roadmap);

    if format == Format::Json {
        let rendered =
            serde_json::to_string_pretty(&discrepancies).map_err(|cause| Failure::Serialisation { cause })?;
        println!("{rendered}");

        return Ok(if discrepancies.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        });
    }

    if discrepancies.is_empty() {
        println!(
            "The register and the roadmap agree: {} probes across {} milestones.",
            register.probes().len(),
            roadmap.sections().len()
        );

        return Ok(ExitCode::SUCCESS);
    }

    println!("The register and the roadmap disagree:");

    for discrepancy in &discrepancies {
        println!("  {discrepancy}");
    }

    println!();
    println!(
        "A probe nobody can find from the roadmap is a question nobody will answer. Name it in its \
         milestone, or move it to the milestone that carries it."
    );

    Ok(ExitCode::FAILURE)
}
