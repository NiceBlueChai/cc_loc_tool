use gpui::{
    div, prelude::*, px, Context, Entity, IntoElement, ParentElement, Render, Styled, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    scroll::ScrollbarAxis,
    theme::ActiveTheme,
    v_flex, Disableable, StyledExt,
};
use std::{collections::HashSet, path::PathBuf, sync::Arc};

use crate::config::AppConfig;
use crate::export::{export_results, ExportFormat};
use crate::loc::{scan_directory, FileLoc, LocSummary, Language};

use super::state::{ScanProgress, ScanState, SortColumn, SortOrder, Theme};

pub struct LocToolView {
    selected_path: Option<PathBuf>,
    exclude_input: Entity<InputState>,
    exclude_files_input: Entity<InputState>,
    scan_state: ScanState,
    scan_progress: ScanProgress,
    results: Vec<FileLoc>,
    summary: LocSummary,
    error_message: Option<String>,
    sort_column: SortColumn,
    sort_order: SortOrder,
    selected_languages: Vec<Language>,
    config: AppConfig,
    theme: Theme,
}

impl LocToolView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 加载配置文件
        let config = match AppConfig::load() {
            Ok(config) => config,
            Err(e) => {
                eprintln!("加载配置失败: {}, 使用默认配置", e);
                AppConfig::default()
            }
        };

        // 根据配置设置排除目录的默认值
        let exclude_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(config.exclude_dirs_to_string())
                .placeholder("输入要排除的目录名，用逗号或分号分隔...")
        });

        // 根据配置设置排除文件的默认值
        let exclude_files_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(config.exclude_files_to_string())
                .placeholder("排除文件，支持通配符 * ，如: moc_*,*.generated.cpp")
        });

        // 先获取theme值，避免移动config后无法访问
        let theme = config.theme;
        
        Self {
            selected_path: config.last_selected_path.clone(),
            exclude_input,
            exclude_files_input,
            scan_state: ScanState::Idle,
            scan_progress: ScanProgress { total_files: 0, processed_files: 0 },
            results: Vec::new(),
            summary: LocSummary::default(),
            error_message: None,
            sort_column: SortColumn::Path,
            sort_order: SortOrder::Asc,
            selected_languages: config.get_selected_languages(),
            config,
            theme,
        }
    }

    fn browse(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let options = gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择 C/C++ 项目目录".into()),
        };

        let receiver = cx.prompt_for_paths(options);

        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                if let Some(path) = paths.into_iter().next() {
                    cx.update(|cx| {
                        this.update(cx, |view, _cx| {
                            view.selected_path = Some(path.clone());
                            // 更新配置并保存
                            view.config.last_selected_path = Some(path);
                            if let Err(e) = view.config.save() {
                                eprintln!("保存配置失败: {}", e);
                            }
                        })
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn scan(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.selected_path.clone() else {
            self.error_message = Some("请先选择目录".into());
            cx.notify();
            return;
        };

        self.scan_state = ScanState::Scanning;
        self.error_message = None;
        self.results.clear();
        self.summary = LocSummary::default();
        self.scan_progress = ScanProgress { total_files: 0, processed_files: 0 };
        cx.notify();

        let path = Arc::new(path);

        // 排除目录
        let exclude_value = self.exclude_input.read(cx).value().to_string();
        let exclude_dirs: HashSet<String> = exclude_value
            .split(|c| c == ',' || c == ';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let exclude_dirs_arc = Arc::new(exclude_dirs.clone());

        // 排除文件
        let exclude_files_value = self.exclude_files_input.read(cx).value().to_string();
        let exclude_files: Vec<String> = exclude_files_value
            .split(|c| c == ',' || c == ';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let exclude_files_arc = Arc::new(exclude_files.clone());
        
        // 更新配置
        self.config.exclude_dirs = exclude_dirs;
        self.config.exclude_files = exclude_files;
        self.config.set_selected_languages(&self.selected_languages);
        
        // 保存配置
        if let Err(e) = self.config.save() {
            eprintln!("保存配置失败: {}", e);
        }

        // 选中的语言
        let selected_languages = self.selected_languages.clone();

        cx.spawn(async move |this, cx| {
            let path_clone = path.clone();
            let exclude_dirs_clone = exclude_dirs_arc.clone();
            let exclude_files_clone = exclude_files_arc.clone();
            let selected_languages_clone = selected_languages.clone();
            
            // 创建一个通道来传递进度信息
            let (progress_sender, progress_receiver) = std::sync::mpsc::channel();
            
            // 在后台执行扫描
            let result = cx
                .background_spawn(async move {
                    scan_directory(
                        &path_clone,
                        &exclude_dirs_clone,
                        &exclude_files_clone,
                        &selected_languages_clone,
                        Some(&|processed, total| {
                            // 发送进度信息
                            let _ = progress_sender.send((processed, total));
                        })
                    )
                })
                .await;
            
            // 处理进度信息 - 简化实现，避免复杂的线程间通信
            // 注意：由于gpui框架的限制，我们将在扫描完成后更新进度
            // 完整的进度更新需要更复杂的框架集成

            cx.update(|cx| {
                this.update(cx, |view, cx| {
                    match result {
                        Ok(files) => {
                            view.summary = LocSummary::from_files(&files);
                            view.results = files;
                            view.scan_state = ScanState::Done;
                        }
                        Err(e) => {
                            view.error_message = Some(format!("扫描失败: {}", e));
                            view.scan_state = ScanState::Error;
                        }
                    }
                    cx.notify();
                })
            })
            .ok();
        })
        .detach();
    }

    fn export(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let options = gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择导出文件路径（自动根据扩展名选择格式）".into()),
        };

        let receiver = cx.prompt_for_paths(options);
        let summary = self.summary.clone();
        let files = self.results.clone();

        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                if let Some(path) = paths.into_iter().next() {
                    // 根据文件扩展名确定导出格式
                    let format = match path.extension().and_then(|ext| ext.to_str()) {
                        Some("csv") => ExportFormat::Csv,
                        Some("json") => ExportFormat::Json,
                        Some("html") => ExportFormat::Html,
                        _ => {
                            // 默认使用CSV格式
                            ExportFormat::Csv
                        }
                    };
                    
                    // 执行导出
                    match export_results(&path, format, &summary, &files) {
                        Ok(_) => {
                            cx.update(|cx| {
                                this.update(cx, |view, cx| {
                                    view.error_message = Some(format!("成功导出到: {:?}", path));
                                    cx.notify();
                                })
                            })
                            .ok();
                        }
                        Err(e) => {
                            cx.update(|cx| {
                                this.update(cx, |view, cx| {
                                    view.error_message = Some(format!("导出失败: {}", e));
                                    cx.notify();
                                })
                            })
                            .ok();
                        }
                    }
                }
            }
        })
        .detach();
    }

    fn render_header(&self, _window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let is_scanning = self.scan_state == ScanState::Scanning;
        let path_display = self
            .selected_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "未选择目录...".to_string());

        v_flex()
            .gap_3()
            .p_4()
            .border_b_1()
            .border_color(cx.theme().border)
            // 第一行：路径选择
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .w(px(80.0))
                            .min_w(px(80.0))
                            .child("项目目录"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_2()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .text_color(if self.selected_path.is_some() {
                                cx.theme().foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(path_display),
                    )
                    .child(
                        Button::new("browse")
                            .label("浏览...")
                            .disabled(is_scanning)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.browse(window, cx);
                            })),
                    )
                    .child(
                        Button::new("scan")
                            .label("扫描")
                            .primary()
                            .loading(is_scanning)
                            .disabled(is_scanning || self.selected_path.is_none() || self.selected_languages.is_empty())
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.scan(window, cx);
                            })),
                    )
                    .child(
                        Button::new("theme-switch")
                            .label(match self.theme {
                                Theme::Light => "深色",
                                Theme::Dark => "浅色",
                            })
                            .on_click(cx.listener(|view, _, _window, cx| {
                                view.toggle_theme(cx);
                            })),
                    ),
            )
            // 第二行：语言选择
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .w(px(80.0))
                            .min_w(px(80.0))
                            .child("选择语言"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(
                                // 全选按钮
                                Button::new("select-all")
                                    .label("全选")
                                    .disabled(is_scanning || self.selected_languages.len() == Language::all().len())
                                    .on_click(cx.listener(|view, _, _window, cx| {
                                        view.selected_languages = Language::all().to_vec();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                // 全不选按钮
                                Button::new("deselect-all")
                                    .label("全不选")
                                    .disabled(is_scanning || self.selected_languages.is_empty())
                                    .on_click(cx.listener(|view, _, _window, cx| {
                                        view.selected_languages.clear();
                                        cx.notify();
                                    })),
                            )
                            // 语言选择按钮
                            .children(Language::all().iter().map(|&language| {
                                let is_selected = self.selected_languages.contains(&language);
                                // 使用语言的变体名称作为固定ID
                                let button_id = match language {
                                    Language::C => "lang-c",
                                    Language::Cpp => "lang-cpp",
                                    Language::Java => "lang-java",
                                    Language::Python => "lang-python",
                                    Language::Go => "lang-go",
                                    Language::Rust => "lang-rust",
                                };
                                
                                let button = Button::new(button_id)
                                    .label(language.display_name())
                                    .disabled(is_scanning);
                                
                                // 根据选择状态设置是否为主要按钮
                                let button = if is_selected {
                                    button.primary()
                                } else {
                                    button
                                };
                                
                                button.on_click(cx.listener(move |view, _, _window, cx| {
                                    view.toggle_language(language, cx);
                                }))
                            })),
                    ),
            )
            // 第三行：排除目录（输入框）
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .w(px(80.0))
                            .min_w(px(80.0))
                            .child("排除目录"),
                    )
                    .child(div().flex_1().child(Input::new(&self.exclude_input))),
            )
            // 第四行：排除文件（输入框）
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .w(px(80.0))
                            .min_w(px(80.0))
                            .child("排除文件"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.exclude_files_input)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("(支持 * 通配符)"),
                    ),
            )
    }

    fn render_summary(&self, _window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        let content = v_flex()
            .gap_4()
            .p_4()
            .bg(theme.muted)
            // 统计卡片行
            .child(
                h_flex()
                    .gap_4()
                    .flex_wrap()
                    .child(self.render_stat_card("文件数", self.summary.files, theme.info, cx))
                    .child(self.render_stat_card("代码行", self.summary.code, theme.success, cx))
                    .child(self.render_stat_card("注释行", self.summary.comments, theme.warning, cx))
                    .child(self.render_stat_card(
                        "空白行",
                        self.summary.blanks,
                        theme.muted_foreground,
                        cx,
                    ))
                    .child(self.render_stat_card("总行数", self.summary.total(), theme.primary, cx))
                    .child(
                        div().ml_auto().child(
                            Button::new("export")
                                .label("导出结果")
                                .primary()
                                .disabled(self.results.is_empty())
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.export(window, cx);
                                })),
                        ),
                    ),
            );
        
        // 只有当有数据时才添加统计图表
        if self.summary.total() > 0 {
            content.child(
                div()
                    .p_4()
                    .bg(theme.background)
                    .rounded(theme.radius)
                    .border_1()
                    .border_color(theme.border)
                    .child(self.render_chart(cx)),
            )
        } else {
            content
        }
    }

    fn render_stat_card(
        &self,
        label: &str,
        value: usize,
        color: gpui::Hsla,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .gap_1()
            .p_3()
            .rounded(theme.radius)
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .min_w(px(100.0))
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(color)
                    .child(format!("{}", value)),
            )
    }

    /// 渲染统计图表（进度条样式）
    fn render_chart(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let total = self.summary.total() as f64;
        
        if total == 0.0 {
            return div().child("无数据可显示");
        }

        // 计算各部分比例
        let code_ratio = self.summary.code as f64 / total;
        let comments_ratio = self.summary.comments as f64 / total;
        let blanks_ratio = self.summary.blanks as f64 / total;

        // 计算百分比
        let code_percent = code_ratio * 100.0;
        let comments_percent = comments_ratio * 100.0;
        let blanks_percent = blanks_ratio * 100.0;

        // 图表宽度
        let chart_width = 300.0;

        v_flex()
            .gap_4()
            .items_center()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(theme.foreground)
                    .child("代码构成比例"),
            )
            // 进度条图表
            .child(
                div()
                    .w(px(chart_width as f32))
                    .h(px(20.0))
                    .rounded(theme.radius)
                    .overflow_hidden()
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .w(px(chart_width as f32))
                            .h(px(20.0))
                            .bg(theme.muted_foreground)
                            .child(
                                div()
                                    .w(px((code_ratio * chart_width) as f32))
                                    .h(px(20.0))
                                    .bg(theme.success)
                                    .child(
                                        div()
                                            .w(px((comments_ratio * chart_width) as f32))
                                            .h(px(20.0))
                                            .ml_auto()
                                            .bg(theme.warning),
                                    ),
                            ),
                    ),
            )
            // 图例
            .child(
                v_flex()
                    .gap_2()
                    .w(px(chart_width as f32))
                    // 代码行图例
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .w(px(12.0))
                                    .h(px(12.0))
                                    .rounded(px(2.0))
                                    .bg(theme.success),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(format!("代码行: {} ({:.1}%)", self.summary.code, code_percent)),
                            )
                    )
                    // 注释行图例
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .w(px(12.0))
                                    .h(px(12.0))
                                    .rounded(px(2.0))
                                    .bg(theme.warning),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(format!("注释行: {} ({:.1}%)", self.summary.comments, comments_percent)),
                            )
                    )
                    // 空白行图例
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .w(px(12.0))
                                    .h(px(12.0))
                                    .rounded(px(2.0))
                                    .bg(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(format!("空白行: {} ({:.1}%)", self.summary.blanks, blanks_percent)),
                            )
                    ),
            )
    }

    fn toggle_sort(&mut self, column: SortColumn, cx: &mut Context<Self>) {
        if self.sort_column == column {
            // 切换排序方向
            self.sort_order = match self.sort_order {
                SortOrder::Asc => SortOrder::Desc,
                SortOrder::Desc => SortOrder::Asc,
            };
        } else {
            self.sort_column = column;
            self.sort_order = SortOrder::Desc; // 默认降序（数字大的在前）
        }
        self.sort_results();
        cx.notify();
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        // 切换主题
        self.theme = match self.theme {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        };
        
        // 更新配置
        self.config.theme = self.theme;
        if let Err(e) = self.config.save() {
            eprintln!("保存主题配置失败: {}", e);
        }
        
        // 重新渲染UI，使主题更改生效
        cx.notify();
    }

    fn sort_results(&mut self) {
        let order = self.sort_order;
        match self.sort_column {
            SortColumn::Path => {
                self.results.sort_by(|a, b| {
                    let cmp = a.path.cmp(&b.path);
                    if order == SortOrder::Asc {
                        cmp
                    } else {
                        cmp.reverse()
                    }
                });
            }
            SortColumn::Code => {
                self.results.sort_by(|a, b| {
                    let cmp = a.code.cmp(&b.code);
                    if order == SortOrder::Asc {
                        cmp
                    } else {
                        cmp.reverse()
                    }
                });
            }
            SortColumn::Comments => {
                self.results.sort_by(|a, b| {
                    let cmp = a.comments.cmp(&b.comments);
                    if order == SortOrder::Asc {
                        cmp
                    } else {
                        cmp.reverse()
                    }
                });
            }
            SortColumn::Blanks => {
                self.results.sort_by(|a, b| {
                    let cmp = a.blanks.cmp(&b.blanks);
                    if order == SortOrder::Asc {
                        cmp
                    } else {
                        cmp.reverse()
                    }
                });
            }
            SortColumn::Total => {
                self.results.sort_by(|a, b| {
                    let cmp = a.total().cmp(&b.total());
                    if order == SortOrder::Asc {
                        cmp
                    } else {
                        cmp.reverse()
                    }
                });
            }
        }
    }

    fn render_header_cell(
        &self,
        label: &'static str,
        column: SortColumn,
        width: Option<gpui::Pixels>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let is_active = self.sort_column == column;
        let indicator = if is_active {
            match self.sort_order {
                SortOrder::Asc => "↑",
                SortOrder::Desc => "↓",
            }
        } else {
            "↓" // 占位符，透明显示
        };

        let cell = h_flex()
            .gap_1()
            .items_center()
            .cursor_pointer()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |view, _, _window, cx| {
                    view.toggle_sort(column, cx);
                }),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(if is_active {
                        cx.theme().primary
                    } else {
                        cx.theme().foreground
                    })
                    .child(label),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(if is_active {
                        cx.theme().primary
                    } else {
                        gpui::transparent_black() // 透明占位
                    })
                    .child(indicator),
            );

        if let Some(w) = width {
            div().w(w).flex().justify_center().child(cell)
        } else {
            div().flex_1().child(cell)
        }
    }

    fn render_results(&self, _window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .p_4()
            .gap_2()
            .child(
                // Table header
                h_flex()
                    .gap_2()
                    .p_2()
                    .bg(theme.muted)
                    .rounded_t(theme.radius)
                    .flex_shrink_0()
                    .child(self.render_header_cell("文件路径", SortColumn::Path, None, cx))
                    .child(self.render_header_cell("代码", SortColumn::Code, Some(px(80.0)), cx))
                    .child(self.render_header_cell(
                        "注释",
                        SortColumn::Comments,
                        Some(px(80.0)),
                        cx,
                    ))
                    .child(self.render_header_cell(
                        "空白",
                        SortColumn::Blanks,
                        Some(px(80.0)),
                        cx,
                    ))
                    .child(self.render_header_cell(
                        "总计",
                        SortColumn::Total,
                        Some(px(80.0)),
                        cx,
                    )),
            )
            .child(
                div()
                    .border_1()
                    .border_color(theme.border)
                    .rounded_b(theme.radius)
                    .child(v_flex().children(self.results.iter().enumerate().map(
                        |(i, file)| {
                            let bg = if i % 2 == 0 {
                                theme.background
                            } else {
                                theme.muted.opacity(0.3)
                            };

                            let path_str = file
                                .path
                                .strip_prefix(
                                    self.selected_path.as_ref().unwrap_or(&PathBuf::new()),
                                )
                                .unwrap_or(&file.path)
                                .to_string_lossy()
                                .to_string();

                            h_flex()
                                .gap_2()
                                .p_2()
                                .bg(bg)
                                .border_b_1()
                                .border_color(theme.border)
                                .child(div().flex_1().text_sm().overflow_x_hidden().child(path_str))
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .text_sm()
                                        .text_center()
                                        .child(format!("{}", file.code)),
                                )
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .text_sm()
                                        .text_center()
                                        .child(format!("{}", file.comments)),
                                )
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .text_sm()
                                        .text_center()
                                        .child(format!("{}", file.blanks)),
                                )
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .text_sm()
                                        .text_center()
                                        .child(format!("{}", file.total())),
                                )
                        },
                    ))),
            )
    }

    fn render_error(&self, _window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        if let Some(ref msg) = self.error_message {
            div()
                .p_3()
                .m_4()
                .rounded(theme.radius)
                .bg(theme.danger.opacity(0.1))
                .border_1()
                .border_color(theme.danger)
                .text_color(theme.danger)
                .child(msg.clone())
        } else {
            div()
        }
    }

    fn render_empty_state(&self, _window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .gap_2()
                    .items_center()
                    .child(div().text_xl().text_color(theme.muted_foreground).child("📂"))
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child("选择一个项目目录开始扫描"),
                    ),
            )
    }

    fn render_progress(&self, _window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let progress = if self.scan_progress.total_files > 0 {
            (self.scan_progress.processed_files as f32 / self.scan_progress.total_files as f32) * 100.0
        } else {
            0.0
        };

        v_flex()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .gap_4()
                    .items_center()
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child("⏳ 正在扫描..."),
                    )
                    .child(
                        div()
                            .w(px(300.0))
                            .h(px(8.0))
                            .bg(theme.muted)
                            .rounded_full()
                            .overflow_hidden()
                            .child(
                                div()
                                    .w(px((progress / 100.0) * 300.0))
                                    .h(px(8.0))
                                    .bg(theme.primary)
                                    .rounded_full(),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("{}/{} 文件", self.scan_progress.processed_files, self.scan_progress.total_files)),
                    ),
            )
    }

    fn toggle_language(&mut self, language: Language, cx: &mut Context<Self>) {
        if let Some(index) = self.selected_languages.iter().position(|&l| l == language) {
            // 如果已经选中，则取消选择
            self.selected_languages.remove(index);
        } else {
            // 如果未选中，则添加选择
            self.selected_languages.push(language);
        }
        cx.notify();
    }
}

impl Render for LocToolView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_results = !self.results.is_empty();
        let is_scanning = self.scan_state == ScanState::Scanning;
        let theme = cx.theme();
        
        // 根据当前主题设置不同的颜色
        let (bg_color, text_color) = match self.theme {
            Theme::Light => (
                theme.background, // 浅色背景
                theme.foreground, // 深色文字
            ),
            Theme::Dark => (
                theme.primary, // 使用深色背景（暂时用primary颜色代替）
                theme.background, // 使用背景色作为文字颜色
            ),
        };

        // 回到使用gpui框架的scrollable方法
        v_flex()
            .size_full()
            .bg(bg_color)
            .text_color(text_color)
            // 固定的顶部内容
            .child(self.render_header(window, cx))
            .child(self.render_error(window, cx))
            // 可滚动的主内容区域
            .child(
                v_flex()
                    .flex_1() // 占据剩余空间
                    .min_h_0() // 确保容器可以收缩
                    .scrollable(ScrollbarAxis::Vertical) // 使用gpui-component的Scrollable组件
                    .child(
                        v_flex()
                            .p_4()
                            .pb_64() // 大幅增加底部padding，确保所有内容都能完全显示
                            .when(has_results, |this|
                                this.child(self.render_summary(window, cx))
                                    .child(self.render_results(window, cx))
                            )
                            .when(is_scanning, |this|
                                this.child(self.render_progress(window, cx))
                            )
                            .when(!has_results && !is_scanning, |this|
                                this.child(self.render_empty_state(window, cx))
                            )
                    )
            )
    }
}