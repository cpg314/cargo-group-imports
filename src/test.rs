use std::collections::HashSet;
use std::io::Write;

use crate::{process_file, Flags};

#[test]
fn test() -> anyhow::Result<()> {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();

    let contents = r#"//! Module-level comment
use std::collections::HashMap;
#[cfg(test)]
use tokio::sync::Mutex;
// Write
use std::io::Write;
pub mod test2;
pub use test2::*;
use std::sync::Arc;
use package::test;
use other_package::test;
use super::test;
use crate::test;

use std::io::Read;

macro_rules! macro {
}
pub use macro;

"#;
    let expected = r#"//! Module-level comment
pub mod test2;
pub use test2::*;

use std::collections::HashMap;
// Write
use std::io::Write;
use std::sync::Arc;
use std::io::Read;

#[cfg(test)]
use tokio::sync::Mutex;

use other_package::test;

use package::test;
use super::test;
use crate::test;

macro_rules! macro {
}
pub use macro;

"#;

    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(contents.as_bytes())?;
    file.flush()?;

    let process = |fix| {
        process_file(
            file.path(),
            "package",
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
    let modified = std::fs::read_to_string(file.path())?;
    assert_eq!(modified, expected);
    Ok(())
}
