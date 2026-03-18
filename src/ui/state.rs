use serde::{Deserialize, Serialize};

/// 复杂度详情弹窗状态
#[derive(Clone, Debug, Default)]
pub struct ComplexityDetailState {
    /// 当前展开详情查看的文件路径
    pub expanded_file: Option<std::path::PathBuf>,
}

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
    Complexity,
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
