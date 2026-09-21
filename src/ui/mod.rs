mod actions;
mod detail;
mod preview;
mod results;
mod state;
mod view;
mod windows;

use gpui_kit::component::{
    ActiveTheme as _,
    scroll::{Scrollbar, ScrollbarMode},
    table::{TableDelegate, TableState},
    theme::Theme as UiTheme,
};
use gpui_kit::{Anchor, App, Entity, IntoElement, ParentElement as _, Pixels, Styled as _, div};

pub use state::Theme;
pub use view::LocToolView;

/// 界面基准字号。所有尺寸都以它换算成 rem，字号变了布局跟着变，不写死像素。
pub(crate) fn ui_rem(cx: &App) -> Pixels {
    cx.theme().font_size
}

/// 表格自带的滚动条是绝对定位压在表格边上的：行高只有二十几像素，内容一超出，
/// 横条就会盖住最后一行的下半截，竖条也会压住最右一列的数字。这里把两条滚动条都
/// 挪到表格外面的边距里，装不下才画出来，画出来也压不到任何内容。
///
/// 表格自己要先关掉内置滚动条（`scrollbar_visible(false, false)`），
/// 这里用同一条滚动句柄重画，拖动、点击轨道的行为不变。
///
/// 结构上不能给表格加一层 flex 容器：`div()` 默认是块级（`display: block`），
/// 表格是 `size_full()`，只有父级的高度是确定值时才能按百分比铺满，
/// 中间多一层块级 div 会让高度塌成 0。所以表格还是本容器的直接子节点，
/// 高度由本容器的 `flex_1` 决定，两条滚动条绝对定位落在 `pr`/`pb` 留出的空档里。
pub(crate) fn table_scroll_area<D: TableDelegate>(
    table: &Entity<TableState<D>>,
    body: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    let state = table.read(cx);
    let horizontal = state.horizontal_scroll_handle.clone();
    let vertical = state.vertical_scroll_handle.clone();
    let thickness = Scrollbar::width();

    div()
        .relative()
        .flex_1()
        .min_h_0()
        .pr(thickness)
        .pb(thickness)
        .child(body)
        .child(
            // 竖条：从顶端到表格底边，横向占右侧那 16px 空档。
            div()
                .absolute()
                .top_0()
                .right_0()
                .bottom(thickness)
                .w(thickness)
                .child(
                    Scrollbar::vertical(&vertical)
                        .viewport_from_layout()
                        .styles(|styles| styles.track(|track| track.width(thickness))),
                ),
        )
        .child(
            // 横条：从左端到表格右边，纵向占底部那 16px 空档。右下角两段留白不相交。
            div()
                .absolute()
                .left_0()
                .right(thickness)
                .bottom_0()
                .h(thickness)
                .child(
                    Scrollbar::horizontal(&horizontal)
                        .viewport_from_layout()
                        .styles(|styles| styles.track(|track| track.width(thickness))),
                ),
        )
}

/// 注册快捷键、应用菜单与动作绑定。必须在创建窗口之前调用。
pub fn init(cx: &mut App) {
    actions::init(cx);
    actions::install_app_menus(cx);
    // 结果表列表宽，装不下的列要被看见：默认的「滚动时淡入」不提示「还有内容」，
    // 这里改成常驻。组件只在内容超出容器时才画滚动条，装得下就自动没有，
    // 所以不需要我们自己去比尺寸。
    UiTheme::set_scrollbar_mode(ScrollbarMode::Always, cx);
    // 默认位置在右上角，正好盖住「导出 / 保存快照 / 历史对比」。挪到右下角，
    // 那里只有列表的最后几行，不会挡住要接着点的按钮。
    UiTheme::global_mut(cx).notification.placement = Anchor::BottomRight;
}
