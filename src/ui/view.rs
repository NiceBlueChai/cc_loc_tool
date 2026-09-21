use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, TryRecvError},
    },
    time::Duration,
};

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, WindowExt as _,
    alert::Alert,
    button::{Button, ButtonVariants as _},
    chart::PieChart,
    checkbox::Checkbox,
    empty::{
        Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant,
        EmptyTitle,
    },
    form::{Field, Form},
    h_flex, h_resizable,
    input::{Input, InputState},
    menu::{AppMenuBar, DropdownMenu as _, PopupMenuItem},
    notification::Notification,
    progress::Progress,
    resizable_panel,
    scroll::ScrollableElement as _,
    switch::Switch,
    table::{DataTable, TableState},
    theme::{Theme as UiTheme, ThemeMode},
    v_flex,
};
use gpui_kit::{
    AnyElement, App, ClipboardItem, Context, Entity, FocusHandle, FontWeight, Hsla, IntoElement,
    ParentElement as _, PathPromptOptions, Render, SharedString, Styled as _, Window, div,
    prelude::*, rems,
};

use crate::complexity::ComplexitySummary;
use crate::config::AppConfig;
use crate::export::{ExportFormat, export_results};
use crate::history::{compare_with_snapshot, create_snapshot, load_snapshot, save_snapshot};
use crate::loc::{
    FileLoc, Language, LocSummary, read_file_content, scan_directory,
    scan_directory_with_complexity,
};

use super::{
    actions, detail, preview,
    results::ResultsTable,
    state::{Notice, ScanProgress, ScanState, Theme},
    table_scroll_area, ui_rem,
    windows::Windows,
};

/// 侧边栏默认宽度（rem）。用户可拖动分隔条调整，这里只是初值。
const SIDEBAR_DEFAULT_REM: f32 = 23.0;
const SIDEBAR_MIN_REM: f32 = 17.0;
const SIDEBAR_MAX_REM: f32 = 34.0;

/// 扫描进度轮询间隔。扫描在工作线程里跑，主线程用它把进度取回来。
const SCAN_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// 语言构成图最多画几个切片，其余合并为「其他」。
const COMPOSITION_SLICES: usize = 4;

const EXPORT_FILE_STEM: &str = "loc-report";
const SNAPSHOT_FILE_NAME: &str = "loc-snapshot.json";

pub struct LocToolView {
    selected_path: Option<PathBuf>,
    exclude_input: Entity<InputState>,
    exclude_files_input: Entity<InputState>,
    custom_extensions_input: Entity<InputState>,
    scan_state: ScanState,
    scan_progress: ScanProgress,
    scan_error: Option<SharedString>,
    /// 每次扫描自增。回调只认自己那一代，避免旧扫描的结果覆盖新扫描。
    scan_generation: u64,
    notice: Option<Notice>,
    summary: LocSummary,
    selected_languages: Vec<Language>,
    analyze_complexity: bool,
    config: AppConfig,
    theme: Theme,
    table: Entity<TableState<ResultsTable>>,
    menu_bar: Entity<AppMenuBar>,
    focus_handle: FocusHandle,
    /// 扫描线程读取的取消标记；置位后工作线程会在下一次进度回调时停下。
    cancel_flag: Arc<AtomicBool>,
    preview_windows: Windows,
    detail_windows: Windows,
}

impl LocToolView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut notice = None;
        let config = match AppConfig::load() {
            Ok(config) => config,
            Err(error) => {
                notice = Some(Notice::error(format!(
                    "读取配置失败，已使用默认设置：{error}"
                )));
                AppConfig::default()
            }
        };

        UiTheme::change(to_theme_mode(config.theme), Some(window), cx);

        let exclude_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(config.exclude_dirs_to_string())
                .placeholder("目录名，用逗号或分号分隔")
        });
        let exclude_files_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(config.exclude_files_to_string())
                .placeholder("支持通配符 *，如 moc_*")
        });
        let custom_extensions_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(config.custom_extensions_to_string())
                .placeholder("如 tpp, ipp, cu")
        });

        let view = cx.weak_entity();
        let table = cx.new(|cx| {
            TableState::new(ResultsTable::new(view), window, cx)
                .col_movable(true)
                .col_resizable(true)
                .row_selectable(true)
                .loop_selection(true)
                .cell_selectable(false)
                .row_header(false)
        });

        let menu_bar = AppMenuBar::new(cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        Self {
            selected_path: config.last_selected_path.clone(),
            exclude_input,
            exclude_files_input,
            custom_extensions_input,
            scan_state: ScanState::Idle,
            scan_progress: ScanProgress::default(),
            scan_error: None,
            scan_generation: 0,
            notice,
            summary: LocSummary::default(),
            selected_languages: config.get_selected_languages(),
            analyze_complexity: config.analyze_complexity,
            theme: config.theme,
            config,
            table,
            menu_bar,
            focus_handle,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            preview_windows: Windows::new(),
            detail_windows: Windows::new(),
        }
    }

    // ---------------------------------------------------------------- 提示

    /// 异步完成的结论走通知，不占布局。
    fn toast(&self, note: Notification, window: &mut Window, cx: &mut Context<Self>) {
        window.push_notification(note, cx);
    }

    /// 打断当前操作的问题（校验失败、读写失败）留在界面上，直到用户处理。
    fn set_notice(&mut self, notice: Notice, cx: &mut Context<Self>) {
        self.notice = Some(notice);
        cx.notify();
    }

    fn dismiss_notice(&mut self, cx: &mut Context<Self>) {
        if self.notice.take().is_some() {
            cx.notify();
        }
    }

    // ---------------------------------------------------------------- 配置

    /// 把输入框里的值写回配置并落盘。输入框是唯一来源，扫描前和任何设置项变更时都会调用。
    fn persist_config(&mut self, cx: &mut Context<Self>) {
        self.config.exclude_dirs = split_terms(&self.exclude_input.read(cx).value())
            .into_iter()
            .collect();
        self.config.exclude_files = split_terms(&self.exclude_files_input.read(cx).value());
        self.config.custom_extensions =
            split_extensions(&self.custom_extensions_input.read(cx).value());
        self.config.set_selected_languages(&self.selected_languages);
        self.config.theme = self.theme;
        self.config.analyze_complexity = self.analyze_complexity;

        if let Err(error) = self.config.save() {
            self.notice = Some(Notice::error(format!("保存配置失败：{error}")));
        }
    }

    fn dialog_directory(&self) -> PathBuf {
        if let Some(path) = &self.selected_path {
            if path.is_dir() {
                return path.clone();
            }
            if let Some(parent) = path.parent() {
                return parent.to_path_buf();
            }
        }

        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    fn project_name(&self) -> SharedString {
        self.selected_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().to_string())
            .filter(|name| !name.is_empty())
            .map(SharedString::new)
            .unwrap_or_else(|| SharedString::new_static("未选择项目"))
    }

    // ---------------------------------------------------------------- 目录

    fn browse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择要统计的项目目录".into()),
        });

        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update_in(cx, |view, _window, cx| view.set_project_path(path, cx))
                    .ok();
            }
        })
        .detach();
    }

    /// 换目录时把上一次的结果、错误与提示一并清掉，避免把旧项目的数字留在新项目上。
    fn set_project_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.selected_path.as_ref() == Some(&path) {
            return;
        }

        self.selected_path = Some(path.clone());
        self.config.last_selected_path = Some(path);

        // 换了目录，在跑的那次扫描已经没有意义：让工作线程尽快停下来，
        // 剩下的进度和结果由代次检查丢掉。
        self.cancel_flag.store(true, Ordering::Relaxed);

        self.scan_state = ScanState::Idle;
        self.scan_progress = ScanProgress::default();
        self.scan_error = None;
        self.notice = None;
        self.summary = LocSummary::default();
        self.table.update(cx, |table, cx| {
            table.delegate_mut().clear();
            table.clear_selection(cx);
            table.refresh(cx);
        });

        self.persist_config(cx);
        cx.notify();
    }

    // ---------------------------------------------------------------- 扫描

    fn scan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.scan_state == ScanState::Scanning {
            return;
        }

        let Some(path) = self.selected_path.clone() else {
            self.set_notice(Notice::error("请先选择要统计的项目目录"), cx);
            return;
        };

        let custom_extensions = split_extensions(&self.custom_extensions_input.read(cx).value());
        if self.selected_languages.is_empty() && custom_extensions.is_empty() {
            self.set_notice(
                Notice::error("请至少选择一种语言，或填写要统计的自定义后缀"),
                cx,
            );
            return;
        }

        self.persist_config(cx);

        let exclude_dirs: HashSet<String> = split_terms(&self.exclude_input.read(cx).value())
            .into_iter()
            .collect();
        let exclude_files = split_terms(&self.exclude_files_input.read(cx).value());
        let languages = self.selected_languages.clone();
        let analyze_complexity = self.analyze_complexity;

        self.scan_state = ScanState::Scanning;
        self.scan_progress = ScanProgress::default();
        self.scan_error = None;
        self.notice = None;
        self.summary = LocSummary::default();
        self.scan_generation = self.scan_generation.wrapping_add(1);
        let generation = self.scan_generation;
        self.table.update(cx, |table, cx| {
            table.delegate_mut().clear();
            table.clear_selection(cx);
            table.refresh(cx);
        });

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = cancel_flag.clone();
        cx.notify();

        let worker_flag = cancel_flag.clone();
        let finished_flag = cancel_flag;
        let (progress_sender, progress_receiver) = mpsc::channel::<ScanProgress>();
        let (result_sender, result_receiver) = mpsc::channel::<anyhow::Result<Vec<FileLoc>>>();

        cx.background_executor()
            .spawn({
                let path = path.clone();
                async move {
                    // 进度回调要求 `Sync`，而 `Sender` 只满足 `Send`，用锁把它藏起来。
                    let sender = Mutex::new(progress_sender);
                    let report = move |processed: usize, total: usize| -> bool {
                        if let Ok(sender) = sender.lock() {
                            let _ = sender.send(ScanProgress {
                                total_files: total,
                                processed_files: processed,
                            });
                        }
                        !worker_flag.load(Ordering::Relaxed)
                    };

                    let outcome = if analyze_complexity {
                        scan_directory_with_complexity(
                            &path,
                            &exclude_dirs,
                            &exclude_files,
                            &languages,
                            &custom_extensions,
                            Some(&report),
                        )
                    } else {
                        scan_directory(
                            &path,
                            &exclude_dirs,
                            &exclude_files,
                            &languages,
                            &custom_extensions,
                            Some(&report),
                        )
                    };

                    let _ = result_sender.send(outcome);
                }
            })
            .detach();

        cx.spawn_in(window, async move |this, cx| {
            loop {
                while let Ok(progress) = progress_receiver.try_recv() {
                    this.update(cx, |view, cx| {
                        if view.scan_generation == generation {
                            view.scan_progress = progress;
                            cx.notify();
                        }
                    })
                    .ok();
                }

                match result_receiver.try_recv() {
                    Ok(outcome) => {
                        let cancelled = finished_flag.load(Ordering::Relaxed);
                        this.update_in(cx, |view, window, cx| {
                            view.finish_scan(outcome, cancelled, generation, &path, window, cx);
                        })
                        .ok();
                        break;
                    }
                    Err(TryRecvError::Disconnected) => {
                        this.update_in(cx, |view, window, cx| {
                            view.finish_scan(
                                Err(anyhow::anyhow!("扫描线程意外中断")),
                                false,
                                generation,
                                &path,
                                window,
                                cx,
                            );
                        })
                        .ok();
                        break;
                    }
                    Err(TryRecvError::Empty) => {
                        cx.background_executor().timer(SCAN_POLL_INTERVAL).await;
                    }
                }
            }
        })
        .detach();
    }

    fn finish_scan(
        &mut self,
        outcome: anyhow::Result<Vec<FileLoc>>,
        cancelled: bool,
        generation: u64,
        base_path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 上一代扫描的收尾不算数：期间可能换了目录或又扫了一次。
        if generation != self.scan_generation {
            return;
        }

        match outcome {
            Ok(files) if cancelled => {
                self.scan_state = ScanState::Cancelled;
                self.apply_results(files, base_path, cx);
                let count = self.table.read(cx).delegate().len();
                self.toast(
                    Notification::warning(format!("扫描已取消，保留已完成的 {count} 个文件")),
                    window,
                    cx,
                );
            }
            Ok(files) => {
                self.scan_state = ScanState::Done;
                self.apply_results(files, base_path, cx);
                let count = self.table.read(cx).delegate().len();
                if count == 0 {
                    self.toast(Notification::info("扫描完成，但没有匹配到文件"), window, cx);
                } else {
                    let code = self.summary.code;
                    self.toast(
                        Notification::success(format!("扫描完成：{count} 个文件，{code} 行代码")),
                        window,
                        cx,
                    );
                }
            }
            Err(error) => {
                self.scan_state = ScanState::Error;
                self.scan_progress = ScanProgress::default();
                self.scan_error = Some(format!("{error}").into());
            }
        }

        cx.notify();
    }

    /// 结果表是唯一数据源，导出、快照、对比都按它的当前顺序取数。
    fn apply_results(&mut self, files: Vec<FileLoc>, base_path: &Path, cx: &mut Context<Self>) {
        self.summary = if files.iter().any(|file| file.complexity.is_some()) {
            LocSummary::from_files_with_complexity(&files)
        } else {
            LocSummary::from_files(&files)
        };

        let previous = self.selected_row_path(cx);
        let base = base_path.to_path_buf();

        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_rows(files, Some(base));
            let target = previous
                .as_deref()
                .and_then(|path| table.delegate().index_of(path));
            table.clear_selection(cx);
            if let Some(row_ix) = target {
                table.set_selected_row(row_ix, cx);
            }
            table.scroll_to_row(target.unwrap_or(0), cx);
            table.refresh(cx);
        });
    }

    /// Esc 在扫描时是取消，扫描结束后是关掉提示，任何时候都有意义。
    fn cancel_or_dismiss(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.scan_state == ScanState::Scanning {
            self.cancel_flag.store(true, Ordering::Relaxed);
            cx.notify();
            return;
        }

        self.dismiss_notice(cx);
    }

    // ---------------------------------------------------------------- 结果

    fn rows(&self, cx: &App) -> Vec<FileLoc> {
        self.table
            .read(cx)
            .delegate()
            .rows()
            .into_iter()
            .cloned()
            .collect()
    }

    fn has_results(&self, cx: &App) -> bool {
        !self.table.read(cx).delegate().is_empty()
    }

    fn selected_row_path(&self, cx: &App) -> Option<PathBuf> {
        let table = self.table.read(cx);
        table
            .selected_row()
            .and_then(|row_ix| table.delegate().row_at(row_ix))
            .map(|file| file.path.clone())
    }

    fn open_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.selected_row_path(cx) {
            Some(path) => self.open_file(&path, window, cx),
            None => self.set_notice(Notice::error("请先在结果列表中选择一个文件"), cx),
        }
    }

    fn show_selected_complexity(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.selected_row_path(cx) {
            Some(path) => self.show_complexity_detail(&path, window, cx),
            None => self.set_notice(Notice::error("请先在结果列表中选择一个文件"), cx),
        }
    }

    fn copy_selected_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.selected_row_path(cx) {
            Some(path) => self.copy_path(&path, window, cx),
            None => self.set_notice(Notice::error("请先在结果列表中选择一个文件"), cx),
        }
    }

    pub fn open_file(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let path = path.to_path_buf();
        let content = cx.background_executor().spawn({
            let path = path.clone();
            async move { read_file_content(&path) }
        });

        cx.spawn_in(window, async move |this, cx| match content.await {
            Ok(content) => {
                this.update_in(cx, |view, _window, cx| {
                    preview::open_preview_window(&path, &content, &mut view.preview_windows, cx);
                })
                .ok();
            }
            Err(error) => {
                this.update_in(cx, |view, window, cx| {
                    view.toast(
                        Notification::error(format!("无法打开文件：{error}")),
                        window,
                        cx,
                    );
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn show_complexity_detail(
        &mut self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let detail = self
            .table
            .read(cx)
            .delegate()
            .rows()
            .into_iter()
            .find(|file| file.path == path)
            .and_then(|file| file.complexity.clone());

        match detail {
            Some(complexity) => {
                detail::open_detail_window(path, &complexity, &mut self.detail_windows, cx);
                let _ = window;
            }
            None => self.set_notice(
                Notice::error("这个文件没有复杂度数据，开启复杂度分析后重新扫描即可查看"),
                cx,
            ),
        }
    }

    pub fn copy_path(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(
            path.to_string_lossy().to_string(),
        ));
        self.toast(Notification::success("已复制文件路径"), window, cx);
    }

    // ---------------------------------------------------------------- 导出与快照

    fn start_export(&mut self, format: ExportFormat, window: &mut Window, cx: &mut Context<Self>) {
        if !self.has_results(cx) {
            self.set_notice(Notice::error("还没有扫描结果可以导出"), cx);
            return;
        }

        let suggested = format!("{EXPORT_FILE_STEM}.{}", format.extension());
        self.prompt_save_path(suggested, window, cx, move |view, path, window, cx| {
            view.export_to(path, format, window, cx);
        });
    }

    fn export_to(
        &mut self,
        path: &Path,
        fallback: ExportFormat,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (target, format) = resolve_export_target(path, fallback);
        let summary = self.summary.clone();
        let files = self.rows(cx);

        match export_results(&target, format, &summary, &files) {
            Ok(()) => self.toast(
                Notification::success(format!("已导出到 {}", target.display())),
                window,
                cx,
            ),
            Err(error) => self.set_notice(Notice::error(format!("导出失败：{error}")), cx),
        }
    }

    fn start_save_snapshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.has_results(cx) {
            self.set_notice(Notice::error("还没有扫描结果可以保存"), cx);
            return;
        }

        let suggested = format!("{}-{SNAPSHOT_FILE_NAME}", self.project_name());
        self.prompt_save_path(suggested, window, cx, |view, path, window, cx| {
            view.save_snapshot_to(path, window, cx);
        });
    }

    fn save_snapshot_to(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let target = resolve_snapshot_target(path);
        let files = self.rows(cx);
        let snapshot = create_snapshot(self.selected_path.as_deref(), &self.summary, &files);

        match save_snapshot(&target, &snapshot) {
            Ok(()) => self.toast(
                Notification::success(format!("已保存快照到 {}", target.display())),
                window,
                cx,
            ),
            Err(error) => self.set_notice(Notice::error(format!("保存快照失败：{error}")), cx),
        }
    }

    fn start_compare_snapshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.has_results(cx) {
            self.set_notice(Notice::error("还没有扫描结果可以对比"), cx);
            return;
        }

        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择要对比的快照文件".into()),
        });

        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update_in(cx, |view, window, cx| view.compare_with(&path, window, cx))
                    .ok();
            }
        })
        .detach();
    }

    fn compare_with(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let baseline = match load_snapshot(path) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.set_notice(Notice::error(format!("读取快照失败：{error}")), cx);
                return;
            }
        };

        let files = self.rows(cx);
        let comparison = compare_with_snapshot(
            self.selected_path.as_deref(),
            &self.summary,
            &files,
            &baseline,
        );

        self.toast(
            Notification::info(format!(
                "新增 {} 个文件，删除 {} 个，变更 {} 个，未变 {} 个；代码 {}，注释 {}，空白 {}，总计 {}",
                comparison.added_files,
                comparison.removed_files,
                comparison.changed_files,
                comparison.unchanged_files,
                signed(comparison.code_delta),
                signed(comparison.comments_delta),
                signed(comparison.blanks_delta),
                signed(comparison.total_delta),
            ))
            .title(format!("对比 {}", baseline.created_at))
            .autohide(false),
            window,
            cx,
        );
    }

    /// 保存对话框只给出建议文件名，真正的落点由用户选定的路径决定。
    fn prompt_save_path(
        &self,
        suggested_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
        run: impl FnOnce(&mut Self, &Path, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let directory = self.dialog_directory();
        let receiver = cx.prompt_for_new_path(&directory, Some(suggested_name.as_str()));

        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(path))) = receiver.await {
                this.update_in(cx, |view, window, cx| run(view, &path, window, cx))
                    .ok();
            }
        })
        .detach();
    }

    // ---------------------------------------------------------------- 设置项

    fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.theme = match self.theme {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        };
        UiTheme::change(to_theme_mode(self.theme), Some(window), cx);
        self.persist_config(cx);
        cx.notify();
    }

    fn set_language(&mut self, language: Language, selected: bool, cx: &mut Context<Self>) {
        self.selected_languages.retain(|item| *item != language);
        if selected {
            self.selected_languages.push(language);
        }
        self.selected_languages.sort_by_key(|item| {
            Language::all()
                .iter()
                .position(|candidate| candidate == item)
                .unwrap_or(usize::MAX)
        });
        self.persist_config(cx);
        cx.notify();
    }

    fn select_all_languages(&mut self, cx: &mut Context<Self>) {
        self.selected_languages = Language::all().to_vec();
        self.persist_config(cx);
        cx.notify();
    }

    fn clear_languages(&mut self, cx: &mut Context<Self>) {
        self.selected_languages.clear();
        self.persist_config(cx);
        cx.notify();
    }

    fn set_analyze_complexity(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.analyze_complexity = enabled;
        self.persist_config(cx);
        cx.notify();
    }

    // ---------------------------------------------------------------- 渲染

    fn render_menu_bar(&self, cx: &Context<Self>) -> AnyElement {
        let rem = ui_rem(cx);
        let border = cx.theme().border;

        div()
            .w_full()
            .flex_shrink_0()
            .h(rems(2.25) * rem)
            .px_2()
            .border_b_1()
            .border_color(border)
            .child(self.menu_bar.clone())
            .into_any_element()
    }

    fn render_notice(&self, notice: Notice, cx: &Context<Self>) -> AnyElement {
        let view = cx.weak_entity();
        let alert = Alert::error("loc-notice", notice.message.clone());

        div()
            .w_full()
            .flex_shrink_0()
            .px_3()
            .pt_3()
            .child(alert.on_close(move |_, _, cx| {
                view.update(cx, |view, cx| view.dismiss_notice(cx)).ok();
            }))
            .into_any_element()
    }

    fn render_sidebar(&self, cx: &Context<Self>) -> AnyElement {
        let border = cx.theme().border;
        let scanning = self.scan_state == ScanState::Scanning;

        v_flex()
            .size_full()
            .min_h_0()
            .child(
                div()
                    .id("loc-sidebar-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(self.render_sidebar_form(cx)),
            )
            .child(
                h_flex()
                    .w_full()
                    .flex_shrink_0()
                    .gap_2()
                    .p_3()
                    .border_t_1()
                    .border_color(border)
                    .child(
                        div().flex_1().child(
                            Button::new("scan-project")
                                .label(if scanning {
                                    "扫描中…"
                                } else {
                                    "开始扫描"
                                })
                                .icon(IconName::Search)
                                .primary()
                                .loading(scanning)
                                .w_full()
                                .on_click(cx.listener(|view, _, window, cx| view.scan(window, cx))),
                        ),
                    )
                    .children(scanning.then(|| {
                        Button::new("cancel-scan").label("取消").ghost().on_click(
                            cx.listener(|view, _, window, cx| view.cancel_or_dismiss(window, cx)),
                        )
                    })),
            )
            .into_any_element()
    }

    fn render_sidebar_form(&self, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let has_path = self.selected_path.is_some();
        let path_text = self
            .selected_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| "尚未选择目录".to_string());

        let form = Form::vertical()
            .gap_4()
            .child(
                Field::new().label("项目目录").child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .rounded(theme.radius)
                                .bg(theme.muted)
                                .text_sm()
                                .text_ellipsis_middle()
                                .text_color(if has_path {
                                    theme.foreground
                                } else {
                                    theme.muted_foreground
                                })
                                .child(path_text),
                        )
                        .child(
                            Button::new("browse-project")
                                .label("浏览…")
                                .icon(IconName::FolderOpen)
                                .w_full()
                                .on_click(
                                    cx.listener(|view, _, window, cx| view.browse(window, cx)),
                                ),
                        ),
                ),
            )
            .child(
                Field::new()
                    .label("排除目录")
                    .description("按目录名排除，用逗号或分号分隔")
                    .child(div().w_full().child(Input::new(&self.exclude_input))),
            )
            .child(
                Field::new()
                    .label("排除文件")
                    .description("支持通配符 *，如 moc_*, *.generated.cpp")
                    .child(div().w_full().child(Input::new(&self.exclude_files_input))),
            )
            .child(
                Field::new()
                    .label("自定义后缀")
                    .description("可带或不带点号，如 tpp, ipp, cu")
                    .child(
                        div()
                            .w_full()
                            .child(Input::new(&self.custom_extensions_input)),
                    ),
            )
            .child(
                Field::new()
                    .label("统计语言")
                    .description("不选语言时，需要靠自定义后缀来指定要统计的文件")
                    .child(
                        v_flex()
                            .w_full()
                            .gap_2()
                            .child(h_flex().w_full().flex_wrap().gap_x_3().gap_y_1().children(
                                Language::all().iter().enumerate().map(|(index, language)| {
                                    let language = *language;
                                    Checkbox::new(("language", index))
                                        .label(language.display_name())
                                        .checked(self.selected_languages.contains(&language))
                                        .on_click(cx.listener(
                                            move |view, checked: &bool, _window, cx| {
                                                view.set_language(language, *checked, cx);
                                            },
                                        ))
                                }),
                            ))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("select-all-languages")
                                            .label("全选")
                                            .ghost()
                                            .xsmall()
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.select_all_languages(cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("clear-languages")
                                            .label("清空")
                                            .ghost()
                                            .xsmall()
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.clear_languages(cx)
                                            })),
                                    ),
                            ),
                    ),
            )
            .child(
                Field::new()
                    .label("复杂度分析")
                    .description("统计圈复杂度可以找出需要拆分的函数，代价是扫描更慢")
                    .child(
                        Switch::new("analyze-complexity")
                            .label(if self.analyze_complexity {
                                "已开启"
                            } else {
                                "已关闭"
                            })
                            .checked(self.analyze_complexity)
                            .on_click(cx.listener(|view, checked: &bool, _window, cx| {
                                view.set_analyze_complexity(*checked, cx)
                            })),
                    ),
            );

        div().p_3().child(form).into_any_element()
    }

    fn render_results_panel(&self, cx: &Context<Self>) -> AnyElement {
        let border = cx.theme().border;

        v_flex()
            .size_full()
            .min_h_0()
            .child(self.render_results_header(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .border_t_1()
                    .border_color(border)
                    .child(self.render_results_body(cx)),
            )
            .into_any_element()
    }

    fn render_results_header(&self, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let rem = ui_rem(cx);
        let busy = self.scan_state == ScanState::Scanning;
        let subtitle = self
            .selected_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| "尚未选择项目目录".to_string());

        h_flex()
            .w_full()
            .flex_shrink_0()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_3()
            .p_3()
            .child(
                v_flex()
                    .flex_1()
                    .min_w(rems(12.0) * rem)
                    .gap_1()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .child(self.project_name()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .text_ellipsis_middle()
                            .child(subtitle),
                    ),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap_2()
                    .child(self.render_export_button(busy, cx))
                    .child(
                        Button::new("save-snapshot")
                            .label("保存快照")
                            .icon(IconName::ArrowDown)
                            .ghost()
                            .disabled(busy)
                            .tooltip("把当前结果存成 JSON 快照")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.start_save_snapshot(window, cx)
                            })),
                    )
                    .child(
                        Button::new("compare-snapshot")
                            .label("历史对比")
                            .icon(IconName::RotateCw)
                            .ghost()
                            .disabled(busy)
                            .tooltip("和之前保存的快照比较")
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.start_compare_snapshot(window, cx)
                            })),
                    )
                    .child(
                        Button::new("toggle-theme")
                            .icon(match self.theme {
                                Theme::Light => IconName::Moon,
                                Theme::Dark => IconName::Sun,
                            })
                            .ghost()
                            .tooltip("切换主题")
                            .on_click(
                                cx.listener(|view, _, window, cx| view.toggle_theme(window, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_export_button(&self, busy: bool, cx: &Context<Self>) -> AnyElement {
        let view = cx.weak_entity();

        Button::new("export-results")
            .label("导出")
            .icon(IconName::ArrowDown)
            .outline()
            .disabled(busy)
            .tooltip("导出为 CSV、JSON 或 HTML")
            .dropdown_menu(move |menu, _window, _cx| {
                ExportFormat::all().iter().fold(menu, |menu, format| {
                    let format = *format;
                    let view = view.clone();
                    menu.item(
                        PopupMenuItem::new(format!("{} (.{})", format.name(), format.extension()))
                            .on_click(move |_, window, cx| {
                                view.update(cx, |view, cx| view.start_export(format, window, cx))
                                    .ok();
                            }),
                    )
                })
            })
            .into_any_element()
    }

    fn render_results_body(&self, cx: &Context<Self>) -> AnyElement {
        match self.scan_state {
            ScanState::Idle => self.render_idle(cx),
            ScanState::Scanning => self.render_scanning(cx),
            ScanState::Error => self.render_error(cx),
            ScanState::Done | ScanState::Cancelled => {
                if self.has_results(cx) {
                    self.render_report(cx)
                } else {
                    self.render_no_matches(cx)
                }
            }
        }
    }

    fn render_idle(&self, cx: &Context<Self>) -> AnyElement {
        // 目录会在启动时从配置恢复，也可能刚用「浏览…」选过，所以空状态分两种：
        // 有目录时给的是「开始扫描」，没有时才让人去选目录。
        let (title, description, action) = if self.selected_path.is_some() {
            (
                "可以开始扫描了",
                "点「开始扫描」，查看每个文件的代码、注释与空白行数。",
                Button::new("scan-from-empty")
                    .label("开始扫描")
                    .icon(IconName::Search)
                    .primary()
                    .on_click(cx.listener(|view, _, window, cx| view.scan(window, cx))),
            )
        } else {
            (
                "先选一个项目目录",
                "选好目录后点「开始扫描」，就能看到每个文件的代码、注释与空白行数。",
                Button::new("browse-from-empty")
                    .label("浏览…")
                    .icon(IconName::FolderOpen)
                    .primary()
                    .on_click(cx.listener(|view, _, window, cx| view.browse(window, cx))),
            )
        };

        centered(
            Empty::new()
                .header(
                    EmptyHeader::new()
                        .media(
                            EmptyMedia::new()
                                .with_variant(EmptyMediaVariant::Icon)
                                .child(IconName::LayoutDashboard),
                        )
                        .title(EmptyTitle::new().child(title))
                        .description(EmptyDescription::new().child(description)),
                )
                .content(EmptyContent::new().child(action))
                .into_any_element(),
            cx,
        )
    }

    fn render_scanning(&self, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let rem = ui_rem(cx);
        let progress = self.scan_progress;
        let enumerating = progress.total_files == 0;
        let label = if enumerating {
            "正在枚举文件…".to_string()
        } else {
            format!(
                "{} / {} 个文件（{:.0}%）",
                grouped(progress.processed_files),
                grouped(progress.total_files),
                progress.percent()
            )
        };

        centered(
            v_flex()
                .w_full()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(label),
                )
                .child(
                    div().w_full().max_w(rems(24.0) * rem).child(
                        Progress::new("loc-progress")
                            .value(progress.percent())
                            .loading(enumerating),
                    ),
                )
                .into_any_element(),
            cx,
        )
    }

    fn render_error(&self, cx: &Context<Self>) -> AnyElement {
        let rem = ui_rem(cx);
        let message = self
            .scan_error
            .clone()
            .unwrap_or_else(|| SharedString::new_static("扫描失败，但没有拿到错误信息"));

        centered(
            v_flex()
                .w_full()
                .items_center()
                .gap_3()
                .child(Alert::error("scan-error", message).max_w(rems(28.0) * rem))
                .child(
                    Button::new("retry-scan")
                        .label("重试")
                        .icon(IconName::RotateCw)
                        .primary()
                        .on_click(cx.listener(|view, _, window, cx| view.scan(window, cx))),
                )
                .into_any_element(),
            cx,
        )
    }

    fn render_no_matches(&self, cx: &Context<Self>) -> AnyElement {
        let cancelled = self.scan_state == ScanState::Cancelled;

        centered(
            Empty::new()
                .header(
                    EmptyHeader::new()
                        .media(
                            EmptyMedia::new()
                                .with_variant(EmptyMediaVariant::Icon)
                                .child(IconName::FileText),
                        )
                        .title(EmptyTitle::new().child(if cancelled {
                            "扫描已取消"
                        } else {
                            "没有匹配的文件"
                        }))
                        .description(EmptyDescription::new().child(if cancelled {
                            "扫描被取消，暂时没有统计到文件。"
                        } else {
                            "当前语言、后缀与排除规则下没有统计到代码文件，检查左侧设置后重新扫描。"
                        })),
                )
                .content(
                    EmptyContent::new().child(
                        Button::new("rescan-from-empty")
                            .label("重新扫描")
                            .icon(IconName::Search)
                            .primary()
                            .on_click(cx.listener(|view, _, window, cx| view.scan(window, cx))),
                    ),
                )
                .into_any_element(),
            cx,
        )
    }

    fn render_report(&self, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let cancelled = self.scan_state == ScanState::Cancelled;
        let complexity_note = self.summary.complexity.as_ref().map(|complexity| {
            div()
                .flex_shrink_0()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(complexity_footnote(complexity))
        });

        v_flex()
            .size_full()
            .min_h_0()
            .gap_3()
            .p_3()
            .children(
                cancelled.then(|| {
                    Alert::warning("loc-cancelled", "上次扫描被取消，下面是已经完成的部分。")
                }),
            )
            .child(self.render_statistics(cx))
            .children(self.render_composition(cx))
            .child(table_scroll_area(
                &self.table,
                DataTable::new(&self.table)
                    .small()
                    .stripe(true)
                    .scrollbar_visible(false, false),
                cx,
            ))
            .children(complexity_note)
            .into_any_element()
    }

    fn render_statistics(&self, cx: &Context<Self>) -> AnyElement {
        h_flex()
            .w_full()
            .flex_shrink_0()
            .flex_wrap()
            .items_start()
            .gap_2()
            .children(self.stat_tiles(cx))
            .into_any_element()
    }

    fn stat_tiles(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme();
        let summary = &self.summary;
        let mut tiles = vec![
            stat_tile("文件", grouped(summary.files), None, cx),
            stat_tile("代码行", grouped(summary.code), None, cx),
            stat_tile("注释行", grouped(summary.comments), None, cx),
            stat_tile("空白行", grouped(summary.blanks), None, cx),
            stat_tile("总计", grouped(summary.total()), None, cx),
        ];

        if let Some(complexity) = &summary.complexity {
            tiles.push(stat_tile(
                "平均复杂度",
                format!("{:.1}", complexity.avg_cyclomatic),
                None,
                cx,
            ));
            tiles.push(stat_tile(
                "函数总数",
                grouped(complexity.total_functions),
                None,
                cx,
            ));
            tiles.push(stat_tile(
                "高复杂度函数",
                grouped(complexity.high_complexity_functions),
                (complexity.high_complexity_functions > 0).then_some(theme.danger),
                cx,
            ));
            tiles.push(stat_tile(
                "长函数",
                grouped(complexity.long_functions),
                (complexity.long_functions > 0).then_some(theme.warning),
                cx,
            ));
        }

        tiles
    }

    fn render_composition(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let slices = self.composition(cx);
        if slices.len() < 2 {
            return None;
        }

        let theme = cx.theme();
        let rem = ui_rem(cx);
        let total: usize = slices.iter().map(|slice| slice.lines).sum();

        let chart = div()
            .flex_shrink_0()
            .w(rems(9.0) * rem)
            .h(rems(9.0) * rem)
            .child(
                PieChart::new(slices.clone())
                    .id("loc-composition")
                    .inner_radius((rems(2.4) * rem).as_f32())
                    .outer_radius((rems(4.2) * rem).as_f32())
                    .value(|slice| slice.lines as f32)
                    .color(|slice| slice.color),
            );

        let legend = v_flex()
            .flex_1()
            .min_w_0()
            .gap_1()
            .children(slices.iter().map(|slice| {
                let percent = if total == 0 {
                    0.0
                } else {
                    slice.lines as f64 / total as f64 * 100.0
                };

                h_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        div()
                            .flex_shrink_0()
                            .w(rems(0.75) * rem)
                            .h(rems(0.75) * rem)
                            .rounded_full()
                            .bg(slice.color),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_ellipsis_middle()
                            .child(slice.label.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("{} 行 · {:.0}%", grouped(slice.lines), percent)),
                    )
            }));

        Some(
            h_flex()
                .w_full()
                .flex_shrink_0()
                .items_center()
                .gap_4()
                .p_3()
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.border)
                .child(chart)
                .child(legend)
                .into_any_element(),
        )
    }

    /// 按后缀把代码行归堆，返回降序的前几项加上「其他」。
    fn composition(&self, cx: &App) -> Vec<CompositionSlice> {
        let theme = cx.theme();
        let palette = [
            theme.chart_1,
            theme.chart_2,
            theme.chart_3,
            theme.chart_4,
            theme.chart_5,
        ];

        let mut totals: BTreeMap<String, usize> = BTreeMap::new();
        for file in self.rows(cx) {
            *totals.entry(extension_label(&file.path)).or_default() += file.code;
        }

        let mut ranked: Vec<(String, usize)> = totals.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let mut slices = Vec::new();
        let mut other = 0usize;
        for (index, (label, lines)) in ranked.into_iter().enumerate() {
            if index < COMPOSITION_SLICES {
                slices.push(CompositionSlice {
                    label: SharedString::new(label),
                    lines,
                    color: palette[index % palette.len()],
                });
            } else {
                other += lines;
            }
        }

        if other > 0 {
            slices.push(CompositionSlice {
                label: SharedString::new_static("其他"),
                lines: other,
                color: palette[COMPOSITION_SLICES % palette.len()],
            });
        }

        slices
    }
}

impl Render for LocToolView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let background = theme.background;
        let foreground = theme.foreground;
        let rem = ui_rem(cx);

        v_flex()
            .size_full()
            .min_h_0()
            .key_context(actions::CONTEXT)
            .track_focus(&self.focus_handle)
            .bg(background)
            .text_color(foreground)
            .child(self.render_menu_bar(cx))
            .children(
                self.notice
                    .clone()
                    .map(|notice| self.render_notice(notice, cx)),
            )
            .child(
                div().flex_1().min_h_0().child(
                    h_resizable("loc-tool-split")
                        .child(
                            resizable_panel()
                                .size(rems(SIDEBAR_DEFAULT_REM) * rem)
                                .size_range(
                                    rems(SIDEBAR_MIN_REM) * rem..rems(SIDEBAR_MAX_REM) * rem,
                                )
                                .child(self.render_sidebar(cx)),
                        )
                        .child(resizable_panel().child(self.render_results_panel(cx))),
                ),
            )
            // 通知层要由内容视图自己放进树里：window.push_notification 只是往
            // Root 里排队，Root 自己不画，不挂上去 toast 永远不出现。
            .children(gpui_kit::component::Root::render_notification_layer(
                window, cx,
            ))
            .on_action(
                cx.listener(|view, _: &actions::BrowseProject, window, cx| view.browse(window, cx)),
            )
            .on_action(
                cx.listener(|view, _: &actions::ScanProject, window, cx| view.scan(window, cx)),
            )
            .on_action(cx.listener(|view, _: &actions::CancelScan, window, cx| {
                view.cancel_or_dismiss(window, cx)
            }))
            .on_action(cx.listener(|view, _: &actions::ExportResults, window, cx| {
                view.start_export(ExportFormat::Csv, window, cx)
            }))
            .on_action(cx.listener(|view, _: &actions::SaveSnapshot, window, cx| {
                view.start_save_snapshot(window, cx)
            }))
            .on_action(
                cx.listener(|view, _: &actions::CompareSnapshot, window, cx| {
                    view.start_compare_snapshot(window, cx)
                }),
            )
            .on_action(cx.listener(|view, _: &actions::ToggleTheme, window, cx| {
                view.toggle_theme(window, cx)
            }))
            .on_action(
                cx.listener(|view, _: &actions::OpenSelectedFile, window, cx| {
                    view.open_selected(window, cx)
                }),
            )
            .on_action(
                cx.listener(|view, _: &actions::ShowSelectedComplexity, window, cx| {
                    view.show_selected_complexity(window, cx)
                }),
            )
            .on_action(
                cx.listener(|view, _: &actions::CopySelectedPath, window, cx| {
                    view.copy_selected_path(window, cx)
                }),
            )
            .on_action(cx.listener(|view, _: &actions::SelectAllLanguages, _, cx| {
                view.select_all_languages(cx)
            }))
            .on_action(
                cx.listener(|view, _: &actions::ClearLanguages, _, cx| view.clear_languages(cx)),
            )
            .on_action(
                cx.listener(|view, _: &actions::DismissNotice, _, cx| view.dismiss_notice(cx)),
            )
    }
}

/// 语言构成图的一段。
#[derive(Clone)]
struct CompositionSlice {
    label: SharedString,
    lines: usize,
    color: Hsla,
}

/// 把内容放进结果区的正中间，并限制一个可读的最大宽度。
fn centered(content: AnyElement, cx: &App) -> AnyElement {
    let rem = ui_rem(cx);

    h_flex()
        .size_full()
        .items_center()
        .justify_center()
        .p_6()
        .child(
            v_flex()
                .w_full()
                .max_w(rems(30.0) * rem)
                .items_center()
                .gap_4()
                .child(content),
        )
        .into_any_element()
}

fn stat_tile(label: &'static str, value: String, accent: Option<Hsla>, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let rem = ui_rem(cx);

    v_flex()
        .flex_1()
        .min_w(rems(8.0) * rem)
        .gap_1()
        .p_3()
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.border)
        .bg(theme.muted.opacity(0.3))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .text_color(accent.unwrap_or(theme.foreground))
                .child(value),
        )
        .into_any_element()
}

fn complexity_footnote(complexity: &ComplexitySummary) -> String {
    format!(
        "复杂度按函数统计：平均 {:.1}，高复杂度 {} 个，长函数 {} 个。表格里的「最大复杂度」是该文件所有函数中的最高值。",
        complexity.avg_cyclomatic, complexity.high_complexity_functions, complexity.long_functions
    )
}

fn to_theme_mode(theme: Theme) -> ThemeMode {
    match theme {
        Theme::Light => ThemeMode::Light,
        Theme::Dark => ThemeMode::Dark,
    }
}

/// 千分位。数据量大的表里，位数比数字本身更容易读错。
fn grouped(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);

    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }

    out
}

/// 增量只有在变多时才值得强调，0 和负数保持原样。
fn signed(value: isize) -> String {
    if value > 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}

/// 输入框里的分隔符中英文都认，逗号和分号混用也认。
fn split_terms(value: &str) -> Vec<String> {
    value
        .split([',', '，', ';', '；'])
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn split_extensions(value: &str) -> Vec<String> {
    let mut seen = HashSet::new();

    split_terms(value)
        .into_iter()
        .map(|item| item.trim_start_matches('.').to_lowercase())
        .filter(|item| !item.is_empty())
        .filter(|item| seen.insert(item.clone()))
        .collect()
}

/// 用户明确写了 csv/json/html 就按它写，否则落到下拉里选的格式。
fn resolve_export_target(path: &Path, fallback: ExportFormat) -> (PathBuf, ExportFormat) {
    if path.is_dir() {
        let file_name = format!("{EXPORT_FILE_STEM}.{}", fallback.extension());
        return (path.join(file_name), fallback);
    }

    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_lowercase)
        .as_deref()
        .and_then(format_from_extension)
    {
        Some(format) => (path.to_path_buf(), format),
        None => (path.with_extension(fallback.extension()), fallback),
    }
}

fn resolve_snapshot_target(path: &Path) -> PathBuf {
    if path.is_dir() {
        return path.join(SNAPSHOT_FILE_NAME);
    }

    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("json") => path.to_path_buf(),
        _ => path.with_extension("json"),
    }
}

fn format_from_extension(extension: &str) -> Option<ExportFormat> {
    ExportFormat::all()
        .iter()
        .copied()
        .find(|format| format.extension() == extension)
}

/// 语言构成图里的分类名：用后缀说话，没有后缀的归到「其他文件」。
fn extension_label(path: &Path) -> String {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if !extension.is_empty() => format!(".{}", extension.to_lowercase()),
        _ => "其他文件".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_terms_accepts_both_separator_styles() {
        assert_eq!(
            split_terms("build, third_party；node_modules; .git，"),
            vec![
                "build".to_string(),
                "third_party".to_string(),
                "node_modules".to_string(),
                ".git".to_string(),
            ]
        );
    }

    #[test]
    fn split_extensions_normalizes_and_dedupes() {
        assert_eq!(
            split_extensions(" .Tpp, ipp; .CU ,, tpp "),
            vec!["tpp".to_string(), "ipp".to_string(), "cu".to_string()]
        );
    }

    #[test]
    fn export_target_keeps_a_known_extension() {
        let (path, format) = resolve_export_target(Path::new("report.html"), ExportFormat::Csv);
        assert_eq!(path, PathBuf::from("report.html"));
        assert_eq!(format, ExportFormat::Html);
    }

    #[test]
    fn export_target_falls_back_to_the_chosen_format() {
        let (path, format) = resolve_export_target(Path::new("report.txt"), ExportFormat::Json);
        assert_eq!(path, PathBuf::from("report.json"));
        assert_eq!(format, ExportFormat::Json);
    }

    #[test]
    fn snapshot_target_is_always_json() {
        assert_eq!(
            resolve_snapshot_target(Path::new("baseline.txt")),
            PathBuf::from("baseline.json")
        );
        assert_eq!(
            resolve_snapshot_target(Path::new("baseline.json")),
            PathBuf::from("baseline.json")
        );
    }

    #[test]
    fn signed_prefixes_only_positive_deltas() {
        assert_eq!(signed(3), "+3");
        assert_eq!(signed(0), "0");
        assert_eq!(signed(-3), "-3");
    }

    #[test]
    fn grouped_inserts_thousands_separators() {
        assert_eq!(grouped(7), "7");
        assert_eq!(grouped(1234), "1,234");
        assert_eq!(grouped(1234567), "1,234,567");
    }
}
