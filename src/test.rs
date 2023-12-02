use std::collections::HashSet;
use std::io::Write;

use crate::{process_file, Flags};

#[test]
fn test() -> anyhow::Result<()> {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();

    // The test-data folder contains a input file with faulty import grouping,
    // and the expected output post-fixing.
    let input = include_bytes!("../test-data/input.rs");
    let expected = include_bytes!("../test-data/output.rs");

    // Write the input to a temporary file
    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(input)?;
    file.flush()?;

    // Processing with or without fixing
    let process = |fix| {
        process_file(
            file.path(),
            "package",
            // Workspace packages
            &HashSet::from(["other_package".to_string()]),
            &Flags {
                workspace: Default::default(),
                rustfmt: false,
                fix,
            },
        )
    };

    // Check
    assert!(process(false)?);
    // Fix
    assert!(process(true)?);
    let modified = std::fs::read(file.path())?;
    assert_eq!(modified, expected);
    // The file is not changed anymore
    assert_eq!(process(true)?, false);
    Ok(())
}
