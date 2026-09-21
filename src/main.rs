use gpui_kit::component::{ActiveTheme as _, Root};
use gpui_kit::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, rems, size};

use cc_loc_tool::ui::{self, LocToolView};

/// 主窗口的初始与最小尺寸，全部按界面字号换算，不写死像素。
const WINDOW_WIDTH_REM: f32 = 74.0;
const WINDOW_HEIGHT_REM: f32 = 48.0;
const WINDOW_MIN_WIDTH_REM: f32 = 52.0;
const WINDOW_MIN_HEIGHT_REM: f32 = 32.0;

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx: &mut App| {
            // 初始化 GPUI Kit（主题、输入、对话框等），再注册本应用的快捷键与菜单。
            gpui_kit::init(cx);
            ui::init(cx);

            let rem = cx.theme().font_size;
            let bounds = Bounds::centered(
                None,
                size(rems(WINDOW_WIDTH_REM) * rem, rems(WINDOW_HEIGHT_REM) * rem),
                cx,
            );

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(
                        rems(WINDOW_MIN_WIDTH_REM) * rem,
                        rems(WINDOW_MIN_HEIGHT_REM) * rem,
                    )),
                    titlebar: Some(TitlebarOptions {
                        title: Some("代码行统计".into()),
                        appears_transparent: false,
                        traffic_light_position: None,
                    }),
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
