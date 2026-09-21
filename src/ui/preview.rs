use std::path::{Path, PathBuf};

use gpui_kit::component::{
    ActiveTheme,
    input::{Editor, EditorState},
    v_flex,
};
use gpui_kit::{
    App, Bounds, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, prelude::*, rems,
};

use super::ui_rem;
use super::windows::Windows;

/// 只读的文件预览窗口。
pub struct FilePreviewView {
    file_path: PathBuf,
    editor: Entity<EditorState>,
}

impl FilePreviewView {
    pub fn new(
        file_path: &Path,
        content: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language(detect_language(file_path).as_str())
                .line_number(true)
                .default_value(content.to_string())
        });

        Self {
            file_path: file_path.to_path_buf(),
            editor,
        }
    }

    /// 文件在磁盘上可能已经变化，重新打开时用最新内容覆盖。
    fn set_content(&mut self, content: &str, window: &mut Window, cx: &mut Context<Self>) {
        let content: SharedString = content.to_string().into();
        self.editor.update(cx, |state, cx| {
            if state.value() != content {
                state.set_value(content, window, cx);
            }
        });
    }
}

impl Render for FilePreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let path_text = self.file_path.to_string_lossy().to_string();

        v_flex()
            .size_full()
            .gap_2()
            .p_3()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .px_1()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(SharedString::from(path_text)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(Editor::new(&self.editor).readonly(true).h_full()),
            )
    }
}

/// 打开文件预览窗口；同一文件已打开时就地刷新内容并聚焦，不重复开窗。
pub fn open_preview_window(file_path: &Path, content: &str, windows: &mut Windows, cx: &mut App) {
    if let Some(existing) = windows.get(file_path) {
        let refreshed = existing
            .update(cx, |root, window, cx| {
                let Ok(view) = root.view().clone().downcast::<FilePreviewView>() else {
                    return false;
                };
                view.update(cx, |view, cx| view.set_content(content, window, cx));
                window.activate_window();
                true
            })
            .unwrap_or(false);
        if refreshed {
            return;
        }
        windows.remove(file_path);
    }

    let title = file_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| file_path.to_string_lossy().to_string());
    let rem = ui_rem(cx);

    let bounds = Bounds::centered(None, gpui_kit::size(rems(40.0) * rem, rems(34.0) * rem), cx);
    let path_key = file_path.to_path_buf();
    let view_path = path_key.clone();
    let content = content.to_string();

    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(gpui_kit::size(rems(20.0) * rem, rems(12.0) * rem)),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            ..Default::default()
        },
        move |window, cx| {
            let view = cx.new(|cx| FilePreviewView::new(&view_path, &content, window, cx));
            cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
        },
    );

    if let Ok(handle) = opened {
        windows.insert(path_key, handle);
    }
}

fn detect_language(path: &Path) -> String {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("c") => "c",
        Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") | Some("h") => "cpp",
        Some("rs") => "rust",
        Some("py") => "python",
        Some("java") => "java",
        Some("go") => "go",
        Some("js") | Some("mjs") | Some("cjs") => "javascript",
        Some("ts") | Some("tsx") => "typescript",
        Some("json") => "json",
        Some("html") => "html",
        Some("css") => "css",
        Some("md") => "markdown",
        Some("toml") => "toml",
        Some("yaml") | Some("yml") => "yaml",
        Some("sh") | Some("bash") => "bash",
        _ => "text",
    }
    .to_string()
}
