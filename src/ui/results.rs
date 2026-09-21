use std::cmp::Ordering;
use std::path::PathBuf;

use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle},
    h_flex,
    menu::{PopupMenu, PopupMenuItem},
    table::{Column, ColumnSort, TableDelegate, TableState},
};
use gpui_kit::{
    App, Context, Div, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, Styled as _, TextAlign, WeakEntity, Window, div,
    prelude::FluentBuilder as _, rems,
};

use crate::complexity::ComplexityLevel;
use crate::loc::FileLoc;

use super::view::LocToolView;

/// 结果表的列。
///
/// 显示顺序由 [`ResultsTable::order`] 持有，用户可以拖动表头调整，所以枚举本身的
/// 顺序只作为初始值。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResultColumn {
    Path,
    Code,
    Comments,
    Blanks,
    Total,
    Complexity,
    Actions,
}

impl ResultColumn {
    pub const ALL: [Self; 7] = [
        Self::Path,
        Self::Code,
        Self::Comments,
        Self::Blanks,
        Self::Total,
        Self::Complexity,
        Self::Actions,
    ];

    fn key(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Code => "code",
            Self::Comments => "comments",
            Self::Blanks => "blanks",
            Self::Total => "total",
            Self::Complexity => "complexity",
            Self::Actions => "actions",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Path => "文件路径",
            Self::Code => "代码",
            Self::Comments => "注释",
            Self::Blanks => "空白",
            Self::Total => "总计",
            Self::Complexity => "最大复杂度",
            Self::Actions => "操作",
        }
    }

    fn is_numeric(self) -> bool {
        matches!(
            self,
            Self::Code | Self::Comments | Self::Blanks | Self::Total | Self::Complexity
        )
    }

    /// 列宽以 rem 为单位。表格按像素宽度排版，且宽度可由用户拖动调整，所以这里给出的是
    /// 随界面字号缩放的初值，而不是写死的像素值。
    ///
    /// 初值取的是「默认窗口宽度下整张表正好放得下」：放不下时靠横向下滚动条兜底，
    /// 但用户不该为了点一下操作列先去滚一次。
    fn width_rem(self) -> f32 {
        match self {
            Self::Path => 20.0,
            Self::Complexity => 6.0,
            Self::Actions => 4.5,
            _ => 4.5,
        }
    }

    fn min_width_rem(self) -> f32 {
        match self {
            Self::Path => 12.0,
            Self::Actions => 4.0,
            _ => 3.5,
        }
    }

    /// 比较两行在该列上的大小，用于排序。
    fn compare(self, a: &FileLoc, b: &FileLoc) -> Ordering {
        match self {
            Self::Path => a.path.cmp(&b.path),
            Self::Code => a.code.cmp(&b.code),
            Self::Comments => a.comments.cmp(&b.comments),
            Self::Blanks => a.blanks.cmp(&b.blanks),
            Self::Total => a.total().cmp(&b.total()),
            Self::Complexity => max_complexity(a).cmp(&max_complexity(b)),
            Self::Actions => Ordering::Equal,
        }
    }
}

fn max_complexity(file: &FileLoc) -> usize {
    file.complexity
        .as_ref()
        .map(|c| c.max_cyclomatic)
        .unwrap_or(0)
}

/// 行悬停分组名。每行必须是独立的名字：分组命中框按名字登记，重名会互相顶掉。
fn row_hover_group(row_ix: usize) -> SharedString {
    SharedString::from(format!("loc-row-hover:{row_ix}"))
}

fn complexity_color(max: usize, cx: &App) -> Hsla {
    let theme = cx.theme();
    match ComplexityLevel::from_complexity(max) {
        ComplexityLevel::Good => theme.foreground,
        ComplexityLevel::Moderate => theme.warning,
        ComplexityLevel::Poor => theme.danger,
    }
}

/// 一行结果。`display_path` 是相对项目根目录的路径，在写入时算好，避免每帧重新拼接。
struct Row {
    loc: FileLoc,
    display_path: SharedString,
}

impl Row {
    fn has_complexity_detail(&self) -> bool {
        self.loc
            .complexity
            .as_ref()
            .is_some_and(|c| !c.functions.is_empty())
    }
}

/// 结果表的数据源与渲染逻辑。
///
/// 排序后的行序就是导出顺序，所以这里同时充当结果列表的唯一来源，视图只通过
/// [`ResultsTable::rows`] 读取。
pub struct ResultsTable {
    rows: Vec<Row>,
    order: Vec<ResultColumn>,
    sort: (ResultColumn, ColumnSort),
    base_path: Option<PathBuf>,
    view: WeakEntity<LocToolView>,
}

impl ResultsTable {
    pub fn new(view: WeakEntity<LocToolView>) -> Self {
        Self {
            rows: Vec::new(),
            order: ResultColumn::ALL.to_vec(),
            sort: (ResultColumn::Path, ColumnSort::Default),
            base_path: None,
            view,
        }
    }

    /// 替换结果集。`base_path` 用于把绝对路径显示成项目内的相对路径。
    ///
    /// 会按当前排序重新排列，避免扫描完成后表头排序标记与实际顺序不一致。
    pub fn set_rows(&mut self, rows: Vec<FileLoc>, base_path: Option<PathBuf>) {
        self.base_path = base_path;
        self.rows = rows
            .into_iter()
            .map(|loc| {
                let display_path = self.display_path(&loc.path);
                Row { loc, display_path }
            })
            .collect();
        self.apply_sort();
    }

    pub fn clear(&mut self) {
        self.rows.clear();
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// 当前显示顺序下的结果，导出与快照都按这个顺序写出。
    pub fn rows(&self) -> Vec<&FileLoc> {
        self.rows.iter().map(|row| &row.loc).collect()
    }

    pub fn row_at(&self, row_ix: usize) -> Option<&FileLoc> {
        self.rows.get(row_ix).map(|row| &row.loc)
    }

    /// 按路径找回行号，用于重新扫描后恢复选中的文件。
    pub fn index_of(&self, path: &std::path::Path) -> Option<usize> {
        self.rows.iter().position(|row| row.loc.path == path)
    }

    fn display_path(&self, path: &std::path::Path) -> SharedString {
        let relative = self
            .base_path
            .as_deref()
            .and_then(|base| path.strip_prefix(base).ok())
            .unwrap_or(path);
        relative.to_string_lossy().to_string().into()
    }

    fn apply_sort(&mut self) {
        let (column, sort) = self.sort;
        self.rows.sort_by(|a, b| match sort {
            ColumnSort::Default => a.loc.path.cmp(&b.loc.path),
            ColumnSort::Ascending => column
                .compare(&a.loc, &b.loc)
                .then_with(|| a.loc.path.cmp(&b.loc.path)),
            ColumnSort::Descending => column
                .compare(&b.loc, &a.loc)
                .then_with(|| a.loc.path.cmp(&b.loc.path)),
        });
    }
}

impl TableDelegate for ResultsTable {
    fn columns_count(&self, _: &App) -> usize {
        self.order.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, cx: &App) -> Column {
        let Some(&column) = self.order.get(col_ix) else {
            return Column::new("", "");
        };

        let rem = cx.theme().font_size;
        let mut col = Column::new(column.key(), column.name())
            .width(rems(column.width_rem()) * rem)
            .min_width(rems(column.min_width_rem()) * rem);

        if column.is_numeric() {
            col = col.text_right();
        }
        if column != ResultColumn::Actions {
            let sort = if self.sort.0 == column {
                self.sort.1
            } else {
                ColumnSort::Default
            };
            col = col.sort(sort);
        } else {
            col = col.selectable(false).resizable(false);
        }

        col
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        let Some(&column) = self.order.get(col_ix) else {
            return;
        };
        self.sort = (column, sort);
        self.apply_sort();
        cx.notify();
    }

    fn render_tr(
        &mut self,
        row_ix: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        div().id(("loc-row", row_ix)).group(row_hover_group(row_ix))
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let (Some(row), Some(&column)) = (self.rows.get(row_ix), self.order.get(col_ix)) else {
            return div().into_any_element();
        };

        match column {
            ResultColumn::Path => h_flex()
                .h_full()
                .items_center()
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .text_ellipsis_middle()
                        .child(row.display_path.clone()),
                )
                .into_any_element(),
            ResultColumn::Complexity => {
                let Some(complexity) = row.loc.complexity.as_ref() else {
                    return h_flex()
                        .h_full()
                        .items_center()
                        .justify_end()
                        .text_color(cx.theme().muted_foreground)
                        .child("—")
                        .into_any_element();
                };
                h_flex()
                    .h_full()
                    .items_center()
                    .justify_end()
                    .text_color(complexity_color(complexity.max_cyclomatic, cx))
                    .child(complexity.max_cyclomatic.to_string())
                    .into_any_element()
            }
            ResultColumn::Actions => {
                if !row.has_complexity_detail() {
                    return div().into_any_element();
                }
                let path = row.loc.path.clone();
                let view = self.view.clone();
                // 鼠标移到行上才现形：复杂度详情在右键菜单、菜单栏和快捷键里都还在，
                // 这里只是把入口放到手边。
                h_flex()
                    .h_full()
                    .items_center()
                    .justify_center()
                    .opacity(0.0)
                    .group_hover(row_hover_group(row_ix), |this| this.opacity(1.0))
                    .child(
                        Button::new(("loc-detail", row_ix))
                            .label("详情")
                            .ghost()
                            .xsmall()
                            .tooltip("查看函数级复杂度")
                            .on_click(move |_, window, cx| {
                                view.update(cx, |view, cx| {
                                    view.show_complexity_detail(&path, window, cx)
                                })
                                .ok();
                            }),
                    )
                    .into_any_element()
            }
            column => {
                let value = match column {
                    ResultColumn::Code => row.loc.code,
                    ResultColumn::Comments => row.loc.comments,
                    ResultColumn::Blanks => row.loc.blanks,
                    ResultColumn::Total => row.loc.total(),
                    _ => 0,
                };
                h_flex()
                    .h_full()
                    .items_center()
                    .justify_end()
                    .child(value.to_string())
                    .into_any_element()
            }
        }
    }

    fn render_th(
        &mut self,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let column = self.column(col_ix, cx);
        div()
            .size_full()
            .flex()
            .items_center()
            .when(column.align == TextAlign::Right, |this| this.justify_end())
            .child(column.name.clone())
    }

    fn context_menu(
        &mut self,
        row_ix: usize,
        menu: PopupMenu,
        _: &mut Window,
        _: &mut Context<gpui_kit::component::table::TableState<Self>>,
    ) -> PopupMenu {
        let Some(row) = self.rows.get(row_ix) else {
            return menu;
        };

        let path = row.loc.path.clone();
        let has_detail = row.has_complexity_detail();

        let open_view = self.view.clone();
        let open_path = path.clone();
        let detail_view = self.view.clone();
        let detail_path = path.clone();
        let copy_view = self.view.clone();
        let copy_path = path;

        menu.item(
            PopupMenuItem::new("打开文件")
                .icon(IconName::FileText)
                .on_click(move |_, window, cx| {
                    open_view
                        .update(cx, |view, cx| view.open_file(&open_path, window, cx))
                        .ok();
                }),
        )
        .item(
            PopupMenuItem::new("复杂度详情")
                .icon(IconName::ChartPie)
                .disabled(!has_detail)
                .on_click(move |_, window, cx| {
                    detail_view
                        .update(cx, |view, cx| {
                            view.show_complexity_detail(&detail_path, window, cx)
                        })
                        .ok();
                }),
        )
        .separator()
        .item(
            PopupMenuItem::new("复制路径")
                .icon(IconName::Copy)
                .on_click(move |_, window, cx| {
                    copy_view
                        .update(cx, |view, cx| view.copy_path(&copy_path, window, cx))
                        .ok();
                }),
        )
    }

    fn render_last_empty_col(
        &mut self,
        _: &mut Window,
        _: &mut Context<gpui_kit::component::table::TableState<Self>>,
    ) -> impl IntoElement {
        // 列宽固定，用弹性空白吸收剩余宽度，让隔行底色与悬停高亮铺满整行。
        h_flex().flex_1().h_full()
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let rem = cx.theme().font_size;
        h_flex()
            .size_full()
            .items_center()
            .justify_center()
            .p_4()
            .child(
                Empty::new().max_w(rems(24.0) * rem).header(
                    EmptyHeader::new()
                        .media(
                            EmptyMedia::new()
                                .with_variant(EmptyMediaVariant::Icon)
                                .child(IconName::FileText),
                        )
                        .title(EmptyTitle::new().child("没有匹配的文件"))
                        .description(
                            EmptyDescription::new().child(
                                "当前语言与排除规则下没有统计到代码文件，检查设置后重新扫描。",
                            ),
                        ),
                ),
            )
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, _: &App) -> String {
        let (Some(row), Some(&column)) = (self.rows.get(row_ix), self.order.get(col_ix)) else {
            return String::new();
        };
        match column {
            ResultColumn::Path => row.display_path.to_string(),
            ResultColumn::Code => row.loc.code.to_string(),
            ResultColumn::Comments => row.loc.comments.to_string(),
            ResultColumn::Blanks => row.loc.blanks.to_string(),
            ResultColumn::Total => row.loc.total().to_string(),
            ResultColumn::Complexity => {
                let max = max_complexity(&row.loc);
                if max == 0 {
                    String::new()
                } else {
                    max.to_string()
                }
            }
            ResultColumn::Actions => String::new(),
        }
    }

    fn move_column(
        &mut self,
        col_ix: usize,
        to_ix: usize,
        _: &mut Window,
        _: &mut Context<gpui_kit::component::table::TableState<Self>>,
    ) {
        if col_ix >= self.order.len() || to_ix >= self.order.len() || col_ix == to_ix {
            return;
        }
        let column = self.order.remove(col_ix);
        self.order.insert(to_ix, column);
    }
}
