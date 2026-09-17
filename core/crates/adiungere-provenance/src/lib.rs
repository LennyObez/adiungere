//! Content Credentials for what adiungere writes, sidecar credentials for what it reads, and a time
//! stamp for either.
//!
//! The product's own manifest says what was seen; this crate makes that manifest something a stranger's
//! validator can check. An output is signed **last**, after every byte of it is final, with the manifest
//! embedded in the container and a hard binding over the rest of the file: any rewrite after that,
//! including one that only moves boxes, is a rewrite the validator reports. An original is never touched,
//! so its manifest goes beside it, as a sidecar the validators look for under the asset's stem; the
//! original stays byte for byte what it was, which is the whole point.
//!
//! Every manifest carries the recordings the asset came from as ingredients, the actions that describe
//! what was done to them in the standard's vocabulary, and the adiungere manifest itself as the
//! `com.adiungere.integrity` assertion. When a time-stamping authority is configured, its token rides in
//! the signature.
//!
//! What is read back says what the validator found and nothing more: a chain that leads to the trust
//! list, a chain that does not, or a manifest that does not hold. A credential generated for the process
//! is on no trust list, and every surface that shows a signature made with it says so.

pub mod credential;
pub mod delegated;
pub mod error;
pub mod sign;
pub mod stamp;
pub mod verify;

pub use credential::{Algorithm, Credential, Kind};
pub use delegated::Delegated;
pub use error::Error;
pub use sign::{GENERATOR, INTEGRITY_ASSERTION, Placement, Request, Signed, sidecar_beside, sign};
pub use stamp::{Attested, Token, check, stamp};
pub use verify::{Credentials, IngredientSummary, SignerIdentity, State, Status, read};
