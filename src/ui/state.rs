use gpui_kit::SharedString;
use serde::{Deserialize, Serialize};

/// 扫描任务的生命周期状态。
///
/// `Done`、`Cancelled`、`Error` 必须与 `Idle` 区分开：三者都会让结果区
/// 不再显示「尚未扫描」的引导文案，而是给出各自的结论。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScanState {
    /// 尚未扫描过
    Idle,
    /// 扫描进行中
    Scanning,
    /// 扫描正常结束（可能为 0 个文件）
    Done,
    /// 用户主动取消
    Cancelled,
    /// 扫描失败
    Error,
}

/// 扫描进度。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ScanProgress {
    pub total_files: usize,
    pub processed_files: usize,
}

impl ScanProgress {
    /// 已完成的百分比，范围 `0.0..=100.0`。
    pub fn percent(&self) -> f32 {
        if self.total_files == 0 {
            0.0
        } else {
            (self.processed_files as f32 / self.total_files as f32) * 100.0
        }
    }
}

/// 内联提示。用于打断当前操作的问题（校验失败、扫描失败等），
/// 异步完成类消息走通知（notification）。
#[derive(Clone, Debug)]
pub struct Notice {
    pub message: SharedString,
}

impl Notice {
    pub fn error(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// 主题类型
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Theme {
    Light,
    Dark,
}
