//! Module-level comment
pub mod test2;
pub use test2::*;

use std::collections::HashMap;
// Write
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex as Mutex2;
use std::io::Read;

#[cfg(test)]
use tokio::sync::Mutex;

use other_package::test;

use package::test;
use crate::test2 as test3;
use super::test;
use crate::test;

macro_rules! test_macro {}
pub use test_macro;
