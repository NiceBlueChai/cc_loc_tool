mod cli;
mod config;
mod export;
mod loc;
mod ui;

use gpui::{prelude::*, px, size, App, Application, Bounds, WindowBounds, WindowOptions};
use gpui_component::init;
use gpui_component::Root;

use ui::LocToolView;

// ============================================================================
// Main Entry
// ============================================================================

fn main() {
    Application::new().run(|cx: &mut App| {
        // Initialize gpui-component (registers themes, inputs, etc.)
        init(cx);

        // Configure window bounds
        let bounds = Bounds::centered(None, size(px(1000.0), px(800.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| LocToolView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .unwrap();
    });
}
