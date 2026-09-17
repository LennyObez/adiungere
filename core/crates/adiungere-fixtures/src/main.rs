//! Writes the synthetic corpus to a directory.
//!
//! ```console
//! $ adiungere-fixtures <directory>
//! ```
//!
//! The directory is created. One file per specification, named after it. The sparse variant is written
//! with its hole left unallocated.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let Some(directory) = arguments.next().map(PathBuf::from) else {
        eprintln!("usage: adiungere-fixtures <directory>");
        return ExitCode::from(2);
    };
    if arguments.next().is_some() {
        eprintln!("usage: adiungere-fixtures <directory>");
        return ExitCode::from(2);
    }

    match adiungere_fixtures::write_corpus(&directory) {
        Ok(written) => {
            for path in written {
                println!("{}", path.display());
            }
            ExitCode::SUCCESS
        },
        Err(error) => {
            eprintln!("adiungere-fixtures: {error}");
            ExitCode::FAILURE
        },
    }
}
