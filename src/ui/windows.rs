use std::collections::HashMap;
use std::path::PathBuf;

use gpui_kit::WindowHandle;
use gpui_kit::component::Root;

/// 已打开的辅助窗口，按文件路径去重，避免同一文件重复开窗。
pub(crate) type Windows = HashMap<PathBuf, WindowHandle<Root>>;
