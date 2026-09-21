use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _,
    clipboard::Clipboard,
    description_list::DescriptionList,
    h_flex,
    table::{Column, ColumnSort, DataTable, TableDelegate, TableState},
    v_flex,
};
use gpui_kit::{
    App, Bounds, Context, Div, Entity, Hsla, IntoElement, ParentElement as _, SharedString,
    Stateful, Styled as _, TextAlign, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
    prelude::*, rems,
};

use crate::complexity::{ComplexityLevel, FileComplexity, FunctionStats};

use super::ui_rem;
use super::windows::Windows;

/// 复杂度详情窗口。函数列表复用结果表组件，表头与单元格因此天然对齐。
pub struct ComplexityDetailView {
    file_path: PathBuf,
    table: Entity<TableState<FunctionTable>>,
}

impl ComplexityDetailView {
    pub fn new(
        file_path: &Path,
        complexity: FileComplexity,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let table = cx.new(|cx| {
            TableState::new(FunctionTable::new(complexity), window, cx)
                .col_movable(false)
                .cell_selectable(false)
                .row_header(false)
        });

        Self {
            file_path: file_path.to_path_buf(),
            table,
        }
    }

    /// 重新扫描后同一文件的复杂度可能变化，就地更新而不是让窗口停留在旧数据上。
    fn set_complexity(&mut self, complexity: FileComplexity, cx: &mut Context<Self>) {
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_complexity(complexity);
            cx.notify();
        });
        cx.notify();
    }
}

impl Render for ComplexityDetailView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let complexity = self.table.read(cx).delegate().complexity();

        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div().font_weight(gpui_kit::FontWeight::BOLD).child(
                                    SharedString::from(
                                        self.file_path
                                            .file_name()
                                            .map(|name| name.to_string_lossy().to_string())
                                            .unwrap_or_else(|| {
                                                self.file_path.to_string_lossy().to_string()
                                            }),
                                    ),
                                ),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .text_ellipsis_middle()
                                    .child(SharedString::from(
                                        self.file_path.to_string_lossy().to_string(),
                                    )),
                            ),
                    )
                    .child(
                        DescriptionList::vertical()
                            .columns(5)
                            .bordered(false)
                            .item("总复杂度", complexity.cyclomatic.to_string(), 1)
                            .item("平均复杂度", format!("{:.1}", complexity.avg_cyclomatic), 1)
                            .item("函数数", complexity.functions.len().to_string(), 1)
                            .item(
                                "高复杂度函数",
                                complexity.high_complexity_count().to_string(),
                                1,
                            )
                            .item(
                                "最长函数",
                                format!("{} 行", complexity.max_function_length),
                                1,
                            ),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .p_3()
                    .child(super::table_scroll_area(
                        &self.table,
                        DataTable::new(&self.table)
                            .stripe(true)
                            .small()
                            .scrollbar_visible(false, false),
                        cx,
                    )),
            )
    }
}

/// 打开复杂度详情窗口；同一文件已打开时就地刷新并聚焦。
pub fn open_detail_window(
    file_path: &Path,
    complexity: &FileComplexity,
    windows: &mut Windows,
    cx: &mut App,
) {
    if let Some(existing) = windows.get(file_path) {
        let refreshed = existing
            .update(cx, |root, window, cx| {
                let Ok(view) = root.view().clone().downcast::<ComplexityDetailView>() else {
                    return false;
                };
                view.update(cx, |view, cx| view.set_complexity(complexity.clone(), cx));
                window.activate_window();
                true
            })
            .unwrap_or(false);
        if refreshed {
            return;
        }
        windows.remove(file_path);
    }

    let title = format!(
        "复杂度详情 - {}",
        file_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| file_path.to_string_lossy().to_string())
    );
    let rem = ui_rem(cx);
    let bounds = Bounds::centered(None, gpui_kit::size(rems(46.0) * rem, rems(32.0) * rem), cx);

    let path_key = file_path.to_path_buf();
    let view_path = path_key.clone();
    let complexity = complexity.clone();

    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(gpui_kit::size(rems(28.0) * rem, rems(16.0) * rem)),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            ..Default::default()
        },
        move |window, cx| {
            let view = cx.new(|cx| ComplexityDetailView::new(&view_path, complexity, window, cx));
            cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
        },
    );

    if let Ok(handle) = opened {
        windows.insert(path_key, handle);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FunctionColumn {
    Name,
    Lines,
    Length,
    Complexity,
    Params,
    Level,
}

impl FunctionColumn {
    const ALL: [Self; 6] = [
        Self::Name,
        Self::Lines,
        Self::Length,
        Self::Complexity,
        Self::Params,
        Self::Level,
    ];

    fn key(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Lines => "lines",
            Self::Length => "length",
            Self::Complexity => "complexity",
            Self::Params => "params",
            Self::Level => "level",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Name => "函数名",
            Self::Lines => "行号",
            Self::Length => "行数",
            Self::Complexity => "复杂度",
            Self::Params => "参数",
            Self::Level => "等级",
        }
    }

    fn width_rem(self) -> f32 {
        match self {
            Self::Name => 24.0,
            Self::Lines => 8.0,
            Self::Complexity => 6.0,
            _ => 5.0,
        }
    }

    fn min_width_rem(self) -> f32 {
        match self {
            Self::Name => 10.0,
            _ => 3.5,
        }
    }

    fn is_numeric(self) -> bool {
        matches!(
            self,
            Self::Length | Self::Complexity | Self::Params | Self::Level
        )
    }

    fn compare(self, a: &FunctionStats, b: &FunctionStats) -> Ordering {
        match self {
            Self::Name => a.name.cmp(&b.name),
            Self::Lines => a.start_line.cmp(&b.start_line),
            Self::Length => a.lines.cmp(&b.lines),
            Self::Complexity => a.cyclomatic.cmp(&b.cyclomatic),
            Self::Params => a.parameter_count.cmp(&b.parameter_count),
            Self::Level => a.cyclomatic.cmp(&b.cyclomatic),
        }
    }
}

fn complexity_color(cyclomatic: usize, cx: &App) -> Hsla {
    let theme = cx.theme();
    match ComplexityLevel::from_complexity(cyclomatic) {
        ComplexityLevel::Good => theme.foreground,
        ComplexityLevel::Moderate => theme.warning,
        ComplexityLevel::Poor => theme.danger,
    }
}

struct FunctionTable {
    complexity: FileComplexity,
    functions: Vec<FunctionStats>,
    sort: (FunctionColumn, ColumnSort),
}

impl FunctionTable {
    fn new(complexity: FileComplexity) -> Self {
        let mut table = Self {
            functions: complexity.functions.clone(),
            complexity,
            sort: (FunctionColumn::Complexity, ColumnSort::Descending),
        };
        table.apply_sort();
        table
    }

    fn set_complexity(&mut self, complexity: FileComplexity) {
        self.functions = complexity.functions.clone();
        self.complexity = complexity;
        self.apply_sort();
    }

    fn complexity(&self) -> &FileComplexity {
        &self.complexity
    }

    fn apply_sort(&mut self) {
        let (column, sort) = self.sort;
        self.functions.sort_by(|a, b| match sort {
            ColumnSort::Default => a.start_line.cmp(&b.start_line),
            ColumnSort::Ascending => column
                .compare(a, b)
                .then_with(|| a.start_line.cmp(&b.start_line)),
            ColumnSort::Descending => column
                .compare(b, a)
                .then_with(|| a.start_line.cmp(&b.start_line)),
        });
    }
}

impl TableDelegate for FunctionTable {
    fn columns_count(&self, _: &App) -> usize {
        FunctionColumn::ALL.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.functions.len()
    }

    fn column(&self, col_ix: usize, cx: &App) -> Column {
        let Some(&column) = FunctionColumn::ALL.get(col_ix) else {
            return Column::new("", "");
        };

        let rem = cx.theme().font_size;
        let mut col = Column::new(column.key(), column.name())
            .width(rems(column.width_rem()) * rem)
            .min_width(rems(column.min_width_rem()) * rem)
            .sort(if self.sort.0 == column {
                self.sort.1
            } else {
                ColumnSort::Default
            });

        if column.is_numeric() {
            col = col.text_right();
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
        let Some(&column) = FunctionColumn::ALL.get(col_ix) else {
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
        div().id(("function-row", row_ix))
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let (Some(function), Some(&column)) =
            (self.functions.get(row_ix), FunctionColumn::ALL.get(col_ix))
        else {
            return div().into_any_element();
        };

        match column {
            FunctionColumn::Name => {
                let signature = if function.parameter_count > 0 {
                    format!("{}({} params)", function.name, function.parameter_count)
                } else {
                    format!("{}()", function.name)
                };
                h_flex()
                    .h_full()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .text_ellipsis()
                            .child(SharedString::from(function.name.clone())),
                    )
                    .child(
                        Clipboard::new(("function-signature", row_ix))
                            .xsmall()
                            .tooltip("复制函数签名")
                            .value(SharedString::from(signature)),
                    )
                    .into_any_element()
            }
            FunctionColumn::Lines => h_flex()
                .h_full()
                .items_center()
                .justify_end()
                .text_color(cx.theme().muted_foreground)
                .child(SharedString::from(format!(
                    "{}-{}",
                    function.start_line, function.end_line
                )))
                .into_any_element(),
            FunctionColumn::Length => h_flex()
                .h_full()
                .items_center()
                .justify_end()
                .child(function.lines.to_string())
                .into_any_element(),
            FunctionColumn::Complexity => h_flex()
                .h_full()
                .items_center()
                .justify_end()
                .font_weight(gpui_kit::FontWeight::BOLD)
                .text_color(complexity_color(function.cyclomatic, cx))
                .child(function.cyclomatic.to_string())
                .into_any_element(),
            FunctionColumn::Params => h_flex()
                .h_full()
                .items_center()
                .justify_end()
                .child(function.parameter_count.to_string())
                .into_any_element(),
            FunctionColumn::Level => {
                let level = ComplexityLevel::from_complexity(function.cyclomatic);
                h_flex()
                    .h_full()
                    .items_center()
                    .justify_end()
                    .text_color(complexity_color(function.cyclomatic, cx))
                    .child(level.name())
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

    fn render_last_empty_col(
        &mut self,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        h_flex().flex_1().h_full()
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, _: &App) -> String {
        let (Some(function), Some(&column)) =
            (self.functions.get(row_ix), FunctionColumn::ALL.get(col_ix))
        else {
            return String::new();
        };
        match column {
            FunctionColumn::Name => function.name.clone(),
            FunctionColumn::Lines => format!("{}-{}", function.start_line, function.end_line),
            FunctionColumn::Length => function.lines.to_string(),
            FunctionColumn::Complexity => function.cyclomatic.to_string(),
            FunctionColumn::Params => function.parameter_count.to_string(),
            FunctionColumn::Level => ComplexityLevel::from_complexity(function.cyclomatic)
                .name()
                .to_string(),
        }
    }
}
