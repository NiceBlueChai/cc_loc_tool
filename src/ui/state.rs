use serde::{Deserialize, Serialize};

/// Scan progress state
#[derive(Clone, Copy, PartialEq)]
pub enum ScanState {
    Idle,
    Scanning,
    Done,
    Error,
}

/// Scan progress information
#[derive(Clone, Copy, PartialEq)]
pub struct ScanProgress {
    pub total_files: usize,
    pub processed_files: usize,
}

/// Column to sort by
#[derive(Clone, Copy, PartialEq)]
pub enum SortColumn {
    Path,
    Code,
    Comments,
    Blanks,
    Total,
}

/// Sort direction
#[derive(Clone, Copy, PartialEq)]
pub enum SortOrder {
    Asc,
    Desc,
}

/// Theme type
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Theme {
    Light,
    Dark,
}
