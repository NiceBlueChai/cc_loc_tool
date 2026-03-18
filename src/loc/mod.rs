mod counter;
mod scanner;

pub use counter::{FileLoc, LocSummary};
pub use scanner::{scan_directory, scan_directory_simple, scan_directory_with_complexity};
pub use crate::language::Language;
