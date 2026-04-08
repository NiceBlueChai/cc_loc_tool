mod counter;
mod scanner;

pub use crate::language::Language;
pub use counter::{FileLoc, LocSummary, read_file_content};
pub use scanner::{scan_directory, scan_directory_simple, scan_directory_with_complexity};
