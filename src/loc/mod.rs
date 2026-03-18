mod counter;
mod scanner;

pub use counter::{FileLoc, LocSummary};
pub use scanner::{Language, scan_directory, scan_directory_simple};
