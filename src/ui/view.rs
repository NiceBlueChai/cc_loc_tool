use gpui::{
    Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px,
    SharedString,
};
use gpui_component::{
    Disableable,
    button::{Button, ButtonVariants},
    clipboard::Clipboard,
    h_flex,
    input::{Input, InputState},
    scroll::ScrollableElement,
    theme::ActiveTheme,
    v_flex,
    Root,
    Sizable,
};
use std::{collections::HashSet, path::PathBuf, sync::Arc};

use crate::config::AppConfig;
use crate::export::{ExportFormat, export_results};
use crate::loc::{FileLoc, Language, LocSummary, scan_directory_simple, scan_directory_with_complexity};

use super::state::{ComplexityDetailState, ScanProgress, ScanState, SortColumn, SortOrder, Theme};

/// 文件预览窗口视图
pub struct FilePreviewView {
    file_path: PathBuf,
    editor: Entity<InputState>,
}

impl FilePreviewView {
    pub fn new(file_path: &PathBuf, content: &str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let language = Self::detect_language(file_path);
        
        let editor = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor(&language)
                .line_number(true)
                .default_value(content.to_string())
        });
        
        Self {
            file_path: file_path.clone(),
            editor,
        }
    }
    
    fn detect_language(path: &PathBuf) -> String {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("c") => "c",
            Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") | Some("h") => "cpp",
            Some("rs") => "rust",
            Some("py") => "python",
            Some("java") => "java",
            Some("go") => "go",
            Some("js") => "javascript",
            Some("ts") => "typescript",
            Some("json") => "json",
            Some("html") => "html",
            Some("css") => "css",
            Some("md") => "markdown",
            Some("toml") => "toml",
            Some("yaml") | Some("yml") => "yaml",
            Some("sh") | Some("bash") => "bash",
            _ => "text",
        }.to_string()
    }
}

impl Render for FilePreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let filename = self
            .file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div()
                    .p_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.muted)
                    .child(
                        h_flex()
                            .items_center()
                            .child(div().font_weight(gpui::FontWeight::BOLD).child(filename)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .p_2()
                    .child(Input::new(&self.editor).h_full()),
            )
    }
}

/// 复杂度详情视图
pub struct ComplexityDetailView {
    file_path: PathBuf,
    complexity: crate::complexity::FileComplexity,
}

impl ComplexityDetailView {
    pub fn new(
        file_path: &PathBuf,
        complexity: crate::complexity::FileComplexity,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            file_path: file_path.clone(),
            complexity,
        }
    }
}

impl Render for ComplexityDetailView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        
        // 按复杂度降序排列函数
        let mut sorted_functions = self.complexity.functions.clone();
        sorted_functions.sort_by(|a, b| b.cyclomatic.cmp(&a.cyclomatic));

        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                // 标题栏
                div()
                    .p_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.muted)
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(format!("复杂度详情 - {}", self.file_path.file_name().unwrap_or_default().to_string_lossy()))
                            )
                            .child(
                                h_flex()
                                    .gap_4()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("总复杂度: {}", self.complexity.cyclomatic))
                                    .child(format!("平均复杂度: {:.1}", self.complexity.avg_cyclomatic))
                                    .child(format!("函数数: {}", self.complexity.functions.len()))
                                    .child(format!("高复杂度函数: {}", self.complexity.high_complexity_count()))
                            )
                    ),
            )
            .child(
                // 函数列表
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_2()
                    .child(
                        v_flex()
                            .gap_1()
                            // 表头
                            .child(
                                h_flex()
                                    .gap_2()
                                    .p_2()
                                    .bg(theme.muted)
                                    .rounded(theme.radius)
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_sm()
                                    .child(div().w(px(200.0)).child("函数名"))
                                    .child(div().w(px(80.0)).text_center().child("行号"))
                                    .child(div().w(px(60.0)).text_center().child("行数"))
                                    .child(div().w(px(80.0)).text_center().child("复杂度"))
                                    .child(div().w(px(60.0)).text_center().child("参数"))
                                    .child(div().w(px(60.0)).text_center().child("状态"))
                            )
                            // 函数行
                            .children(sorted_functions.iter().enumerate().map(|(i, func)| {
                                let bg = if i % 2 == 0 {
                                    theme.background
                                } else {
                                    theme.muted.opacity(0.3)
                                };
                                
                                // 复杂度颜色
                                let (complexity_color, status_text, status_color) = if func.cyclomatic <= 10 {
                                    (theme.success, "良好", theme.success)
                                } else if func.cyclomatic <= 20 {
                                    (theme.warning, "中等", theme.warning)
                                } else {
                                    (theme.danger, "需改进", theme.danger)
                                };

                                // 生成函数签名
                                let signature = if func.parameter_count > 0 {
                                    format!("{}({} params)", func.name, func.parameter_count)
                                } else {
                                    format!("{}()", func.name)
                                };

                                h_flex()
                                    .gap_2()
                                    .p_2()
                                    .bg(bg)
                                    .rounded(theme.radius)
                                    .text_sm()
                                    .items_center()
                                    .child(
                                        // 显示函数名
                                        div()
                                            .w(px(160.0))
                                            .overflow_x_hidden()
                                            .child(func.name.clone())
                                    )
                                    .child(
                                        // 复制按钮（Clipboard 组件自带复制图标）
                                        Clipboard::new(("copy-func", i))
                                            .value(SharedString::from(signature.clone()))
                                    )
                                    .child(
                                        div()
                                            .w(px(80.0))
                                            .text_center()
                                            .text_color(theme.muted_foreground)
                                            .child(format!("{}-{}", func.start_line, func.end_line))
                                    )
                                    .child(
                                        div()
                                            .w(px(60.0))
                                            .text_center()
                                            .child(format!("{}", func.lines))
                                    )
                                    .child(
                                        div()
                                            .w(px(80.0))
                                            .text_center()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(complexity_color)
                                            .child(format!("{}", func.cyclomatic))
                                    )
                                    .child(
                                        div()
                                            .w(px(60.0))
                                            .text_center()
                                            .child(format!("{}", func.parameter_count))
                                    )
                                    .child(
                                        div()
                                            .w(px(60.0))
                                            .text_center()
                                            .text_color(status_color)
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .child(status_text)
                                    )
                            }))
                    )
            )
    }
}

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
    complexity_detail_state: ComplexityDetailState,
    /// 是否启用复杂度分析
    analyze_complexity: bool,
}

impl LocToolView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = match AppConfig::load() {
            Ok(config) => config,
            Err(e) => {
                eprintln!("加载配置失败: {}, 使用默认配置", e);
                AppConfig::default()
            }
        };

        let exclude_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(config.exclude_dirs_to_string())
                .placeholder("输入要排除的目录名，用逗号或分号分隔...")
        });

        let exclude_files_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(config.exclude_files_to_string())
                .placeholder("排除文件，支持通配符 * ，如: moc_*,*.generated.cpp")
        });

        let theme = config.theme;
        let analyze_complexity = config.analyze_complexity;

        Self {
            selected_path: config.last_selected_path.clone(),
            exclude_input,
            exclude_files_input,
            scan_state: ScanState::Idle,
            scan_progress: ScanProgress {
                total_files: 0,
                processed_files: 0,
            },
            results: Vec::new(),
            summary: LocSummary::default(),
            error_message: None,
            sort_column: SortColumn::Path,
            sort_order: SortOrder::Asc,
            selected_languages: config.get_selected_languages(),
            config,
            theme,
            complexity_detail_state: ComplexityDetailState::default(),
            analyze_complexity,
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
        self.scan_progress = ScanProgress {
            total_files: 0,
            processed_files: 0,
        };
        cx.notify();

        let path = Arc::new(path);

        let exclude_value = self.exclude_input.read(cx).value().to_string();
        let exclude_dirs: HashSet<String> = exclude_value
            .split(|c| c == ',' || c == ';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let exclude_dirs_arc = Arc::new(exclude_dirs.clone());

        let exclude_files_value = self.exclude_files_input.read(cx).value().to_string();
        let exclude_files: Vec<String> = exclude_files_value
            .split(|c| c == ',' || c == ';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let exclude_files_arc = Arc::new(exclude_files.clone());

        self.config.exclude_dirs = exclude_dirs;
        self.config.exclude_files = exclude_files;
        self.config.set_selected_languages(&self.selected_languages);

        if let Err(e) = self.config.save() {
            eprintln!("保存配置失败: {}", e);
        }

        let selected_languages = self.selected_languages.clone();
        let analyze_complexity = self.analyze_complexity;

        cx.spawn(async move |this, cx| {
            let path_clone = path.clone();
            let exclude_dirs_clone = exclude_dirs_arc.clone();
            let exclude_files_clone = exclude_files_arc.clone();
            let selected_languages_clone = selected_languages.clone();

            let (progress_sender, _progress_receiver) = std::sync::mpsc::channel();

            // 根据开关选择扫描函数
            let result = if analyze_complexity {
                // 使用带复杂度分析的扫描函数
                cx.background_spawn(async move {
                    scan_directory_with_complexity(
                        &path_clone,
                        &exclude_dirs_clone,
                        &exclude_files_clone,
                        &selected_languages_clone,
                        Some(&|processed, total| {
                            let _ = progress_sender.send((processed, total));
                        }),
                    )
                })
                .await
                .map(|files| {
                    // 使用带复杂度统计的汇总函数
                    (files, true)
                })
            } else {
                // 使用简单扫描函数（更快）
                cx.background_spawn(async move {
                    scan_directory_simple(
                        &path_clone,
                        &exclude_dirs_clone,
                        &exclude_files_clone,
                        &selected_languages_clone,
                    )
                })
                .await
                .map(|files| {
                    // 使用简单汇总函数
                    (files, false)
                })
            };

            cx.update(|cx| {
                this.update(cx, |view, cx| {
                    match result {
                        Ok((files, with_complexity)) => {
                            // 根据扫描类型选择汇总方式
                            view.summary = if with_complexity {
                                LocSummary::from_files_with_complexity(&files)
                            } else {
                                LocSummary::from_files(&files)
                            };
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
                    let format = match path.extension().and_then(|ext| ext.to_str()) {
                        Some("csv") => ExportFormat::Csv,
                        Some("json") => ExportFormat::Json,
                        Some("html") => ExportFormat::Html,
                        _ => ExportFormat::Csv,
                    };

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
                            .disabled(
                                is_scanning
                                    || self.selected_path.is_none()
                                    || self.selected_languages.is_empty(),
                            )
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
                                Button::new("select-all")
                                    .label("全选")
                                    .disabled(
                                        is_scanning
                                            || self.selected_languages.len()
                                                == Language::all().len(),
                                    )
                                    .on_click(cx.listener(|view, _, _window, cx| {
                                        view.selected_languages = Language::all().to_vec();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("deselect-all")
                                    .label("全不选")
                                    .disabled(is_scanning || self.selected_languages.is_empty())
                                    .on_click(cx.listener(|view, _, _window, cx| {
                                        view.selected_languages.clear();
                                        cx.notify();
                                    })),
                            )
                            .children(Language::all().iter().map(|&language| {
                                let is_selected = self.selected_languages.contains(&language);
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
                    .child(div().flex_1().child(Input::new(&self.exclude_files_input)))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("(支持 * 通配符)"),
                    ),
            )
            // 复杂度分析开关
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
                            .child("复杂度分析"),
                    )
                    .child(
                        Button::new("toggle-complexity")
                            .label(if self.analyze_complexity { "✓ 启用" } else { "☐ 禁用" })
                            .disabled(is_scanning)
                            .when(self.analyze_complexity, |this| this.primary())
                            .on_click(cx.listener(|view, _, _window, cx| {
                                view.analyze_complexity = !view.analyze_complexity;
                                view.config.analyze_complexity = view.analyze_complexity;
                                if let Err(e) = view.config.save() {
                                    eprintln!("保存配置失败: {}", e);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("(启用可能增加扫描时间)"),
                    ),
            )
    }

    fn render_summary(&self, _window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        let content = v_flex()
            .gap_4()
            .p_4()
            .bg(theme.muted)
            .child(
                h_flex()
                    .gap_4()
                    .flex_wrap()
                    .child(self.render_stat_card("文件数", self.summary.files, theme.info, cx))
                    .child(self.render_stat_card("代码行", self.summary.code, theme.success, cx))
                    .child(self.render_stat_card(
                        "注释行",
                        self.summary.comments,
                        theme.warning,
                        cx,
                    ))
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
            )
            // 复杂度统计卡片行
            .when_some(self.summary.complexity.as_ref(), |this, complexity| {
                this.child(
                    h_flex()
                        .gap_4()
                        .flex_wrap()
                        .pt_4()
                        .border_t_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(theme.muted_foreground)
                                .child("复杂度分析"),
                        )
                        .child(self.render_stat_card_f64("平均圈复杂度", complexity.avg_cyclomatic, theme.info, cx))
                        .child(self.render_stat_card("函数总数", complexity.total_functions, theme.success, cx))
                        .child(self.render_stat_card(
                            "高复杂度函数",
                            complexity.high_complexity_functions,
                            if complexity.high_complexity_functions > 0 { theme.danger } else { theme.success },
                            cx,
                        ))
                        .child(self.render_stat_card_f64("平均函数长度", complexity.avg_function_length, theme.warning, cx))
                        .child(self.render_stat_card(
                            "长函数(>50行)",
                            complexity.long_functions,
                            if complexity.long_functions > 0 { theme.danger } else { theme.success },
                            cx,
                        )),
                )
            });

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

    fn render_stat_card_f64(
        &self,
        label: &str,
        value: f64,
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
                    .child(format!("{:.1}", value)),
            )
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

    fn render_chart(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let total = self.summary.total() as f64;

        if total == 0.0 {
            return div().child("无数据可显示");
        }

        let code_ratio = self.summary.code as f64 / total;
        let comments_ratio = self.summary.comments as f64 / total;
        let blanks_ratio = self.summary.blanks as f64 / total;

        let code_percent = code_ratio * 100.0;
        let comments_percent = comments_ratio * 100.0;
        let blanks_percent = blanks_ratio * 100.0;

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
            .child(
                v_flex()
                    .gap_2()
                    .w(px(chart_width as f32))
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
                            .child(div().text_sm().text_color(theme.foreground).child(format!(
                                "代码行: {} ({:.1}%)",
                                self.summary.code, code_percent
                            ))),
                    )
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
                            .child(div().text_sm().text_color(theme.foreground).child(format!(
                                "注释行: {} ({:.1}%)",
                                self.summary.comments, comments_percent
                            ))),
                    )
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
                            .child(div().text_sm().text_color(theme.foreground).child(format!(
                                "空白行: {} ({:.1}%)",
                                self.summary.blanks, blanks_percent
                            ))),
                    ),
            )
    }

    fn toggle_sort(&mut self, column: SortColumn, cx: &mut Context<Self>) {
        if self.sort_column == column {
            self.sort_order = match self.sort_order {
                SortOrder::Asc => SortOrder::Desc,
                SortOrder::Desc => SortOrder::Asc,
            };
        } else {
            self.sort_column = column;
            self.sort_order = SortOrder::Desc;
        }
        self.sort_results();
        cx.notify();
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = match self.theme {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        };

        self.config.theme = self.theme;
        if let Err(e) = self.config.save() {
            eprintln!("保存主题配置失败: {}", e);
        }

        cx.notify();
    }

    fn sort_results(&mut self) {
        let order = self.sort_order;
        match self.sort_column {
            SortColumn::Path => {
                self.results.sort_by(|a, b| {
                    let cmp = a.path.cmp(&b.path);
                    if order == SortOrder::Asc { cmp } else { cmp.reverse() }
                });
            }
            SortColumn::Code => {
                self.results.sort_by(|a, b| {
                    let cmp = a.code.cmp(&b.code);
                    if order == SortOrder::Asc { cmp } else { cmp.reverse() }
                });
            }
            SortColumn::Comments => {
                self.results.sort_by(|a, b| {
                    let cmp = a.comments.cmp(&b.comments);
                    if order == SortOrder::Asc { cmp } else { cmp.reverse() }
                });
            }
            SortColumn::Blanks => {
                self.results.sort_by(|a, b| {
                    let cmp = a.blanks.cmp(&b.blanks);
                    if order == SortOrder::Asc { cmp } else { cmp.reverse() }
                });
            }
            SortColumn::Total => {
                self.results.sort_by(|a, b| {
                    let cmp = a.total().cmp(&b.total());
                    if order == SortOrder::Asc { cmp } else { cmp.reverse() }
                });
            }
            SortColumn::Complexity => {
                // 按最大复杂度排序，没有复杂度数据的排在最后
                self.results.sort_by(|a, b| {
                    let a_complexity = a.complexity.as_ref().map(|c| c.max_cyclomatic).unwrap_or(0);
                    let b_complexity = b.complexity.as_ref().map(|c| c.max_cyclomatic).unwrap_or(0);
                    let cmp = a_complexity.cmp(&b_complexity);
                    if order == SortOrder::Asc { cmp } else { cmp.reverse() }
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
            "↓"
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
                        gpui::transparent_black()
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
                    .child(self.render_header_cell("空白", SortColumn::Blanks, Some(px(80.0)), cx))
                    .child(self.render_header_cell("总计", SortColumn::Total, Some(px(80.0)), cx))
                    .child(self.render_header_cell("复杂度", SortColumn::Complexity, Some(px(80.0)), cx))
                    .child(div().w(px(60.0))), // 详情按钮预留空间
            )
            .child(
                div()
                    .border_1()
                    .border_color(theme.border)
                    .rounded_b(theme.radius)
                    .child(
                        v_flex().children(self.results.iter().enumerate().map(|(i, file)| {
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

                            // 复杂度显示：显示最大复杂度，并根据等级着色
                            let (complexity_text, complexity_color) = match &file.complexity {
                                Some(c) => {
                                    let text = format!("{}", c.max_cyclomatic);
                                    // 根据复杂度等级着色
                                    let color = if c.max_cyclomatic <= 10 {
                                        theme.success  // 良好
                                    } else if c.max_cyclomatic <= 20 {
                                        theme.warning  // 中等
                                    } else {
                                        theme.danger   // 需要改进
                                    };
                                    (text, color)
                                }
                                None => ("-".to_string(), theme.muted_foreground),
                            };

                            // 是否有复杂度详情可显示
                            let has_complexity_detail = file.complexity.as_ref()
                                .map(|c| !c.functions.is_empty())
                                .unwrap_or(false);

                            let mut row = h_flex()
                                .gap_2()
                                .p_2()
                                .border_b_1()
                                .border_color(theme.border)
                                .cursor_pointer();

                            let file_path = file.path.clone();
                            row = row.on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(move |view, _, _window, cx| {
                                    view.load_file_content(&file_path, cx);
                                }),
                            );

                            row = row.bg(bg);

                            // 文件路径
                            row.child(div().flex_1().text_sm().overflow_x_hidden().child(path_str))
                                // 代码行
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .text_sm()
                                        .text_center()
                                        .child(format!("{}", file.code)),
                                )
                                // 注释行
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .text_sm()
                                        .text_center()
                                        .child(format!("{}", file.comments)),
                                )
                                // 空白行
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .text_sm()
                                        .text_center()
                                        .child(format!("{}", file.blanks)),
                                )
                                // 总行数
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .text_sm()
                                        .text_center()
                                        .child(format!("{}", file.total())),
                                )
                                // 复杂度
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .text_sm()
                                        .text_center()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(complexity_color)
                                        .child(complexity_text),
                                )
                                // 详情按钮
                                .child(
                                    div()
                                        .w(px(60.0))
                                        .flex()
                                        .justify_center()
                                        .when(has_complexity_detail, |this| {
                                            let file_path_btn = file.path.clone();
                                            this.child(
                                                Button::new(("detail", i))
                                                    .label("详情")
                                                    .xsmall()
                                                    .on_click(cx.listener(move |view, _, _window, cx| {
                                                        view.show_complexity_detail(&file_path_btn, cx);
                                                    }))
                                            )
                                        }),
                                )
                        })),
                    ),
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

        div().flex_1().flex().items_center().justify_center().child(
            v_flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .text_xl()
                        .text_color(theme.muted_foreground)
                        .child("📂"),
                )
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
            (self.scan_progress.processed_files as f32 / self.scan_progress.total_files as f32)
                * 100.0
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
                            .child(format!(
                                "{}/{} 文件",
                                self.scan_progress.processed_files, self.scan_progress.total_files
                            )),
                    ),
            )
    }

    fn toggle_language(&mut self, language: Language, cx: &mut Context<Self>) {
        if let Some(index) = self.selected_languages.iter().position(|&l| l == language) {
            self.selected_languages.remove(index);
        } else {
            self.selected_languages.push(language);
        }
        self.config.set_selected_languages(&self.selected_languages);
        if let Err(e) = self.config.save() {
            eprintln!("保存语言配置失败: {}", e);
        }
        cx.notify();
    }

    /// 显示文件复杂度详情弹窗
    fn show_complexity_detail(&mut self, file_path: &PathBuf, cx: &mut Context<Self>) {
        // 查找该文件的复杂度信息
        let file_loc = self.results.iter().find(|f| &f.path == file_path);
        let complexity = match file_loc.and_then(|f| f.complexity.as_ref()) {
            Some(c) => c.clone(),
            None => return,
        };

        let file_path_clone = file_path.clone();
        
        // 打开新窗口显示复杂度详情
        let bounds = gpui::Bounds::centered(
            None,
            gpui::size(gpui::px(700.0), gpui::px(500.0)),
            cx,
        );

        let _ = cx.open_window(
            gpui::WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(format!("复杂度详情 - {}", file_path.file_name().unwrap_or_default().to_string_lossy()).into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| {
                    ComplexityDetailView::new(&file_path_clone, complexity, window, cx)
                });
                cx.new(|cx| Root::new(view, window, cx))
            },
        );
    }

    fn load_file_content(&mut self, file_path: &PathBuf, cx: &mut Context<Self>) {
        let file_path_clone = file_path.clone();

        cx.spawn(async move |this, cx| {
            let result = std::fs::read_to_string(&file_path_clone);

            cx.update(|cx| {
                match result {
                    Ok(content) => {
                        let bounds = gpui::Bounds::centered(
                            None,
                            gpui::size(gpui::px(800.0), gpui::px(600.0)),
                            cx,
                        );

                        let _ = cx.open_window(
                            gpui::WindowOptions {
                                window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                                ..Default::default()
                            },
                            |window, cx| {
                                let view = cx.new(|cx| {
                                    FilePreviewView::new(&file_path_clone, &content, window, cx)
                                });
                                cx.new(|cx| Root::new(view, window, cx))
                            },
                        );
                    }
                    Err(e) => {
                        let _ = this.update(cx, |view, cx| {
                            view.error_message = Some(format!("无法读取文件: {}", e));
                            cx.notify();
                        });
                    }
                }
            })
            .ok();
        })
        .detach();
    }
}

impl Render for LocToolView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_results = !self.results.is_empty();
        let is_scanning = self.scan_state == ScanState::Scanning;
        let theme = cx.theme();

        let (bg_color, text_color) = match self.theme {
            Theme::Light => (
                theme.background,
                theme.foreground,
            ),
            Theme::Dark => (
                theme.primary,
                theme.background,
            ),
        };

        v_flex()
            .size_full()
            .bg(bg_color)
            .text_color(text_color)
            .child(self.render_header(window, cx))
            .child(self.render_error(window, cx))
            .child(
                v_flex()
                    .id("main-content-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .p_4()
                            .pb_64()
                            .when(has_results, |this| {
                                this.child(self.render_summary(window, cx))
                                    .child(self.render_results(window, cx))
                            })
                            .when(is_scanning, |this| {
                                this.child(self.render_progress(window, cx))
                            })
                            .when(!has_results && !is_scanning, |this| {
                                this.child(self.render_empty_state(window, cx))
                            }),
                    ),
            )
    }
}
