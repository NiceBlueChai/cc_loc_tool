# AGENTS.md

只记录**从代码里看不出来**的约束和踩过的坑。功能清单见 [README.md](README.md)，进度见 [ROADMAP.md](ROADMAP.md)。

## 形态

一个 crate 三个目标：GUI（`src/main.rs`）、CLI（`src/cli_main.rs`）、共用 lib（`src/lib.rs`）。
**新增逻辑一律放 lib**，两个 bin 只做启动；改 lib 会同时影响 GUI 和 CLI，两边都要想过。

## 命令与门禁

- CI 三条门禁：`cargo fmt --check`、`cargo test --all-targets`、`cargo clippy --all-targets --all-features`（要求零警告）。交付前本地跑完再报完成，不靠 CI 兜底。
- 裸 `cargo run` 会失败（两个 bin 且无 `default-run`）。**不要为它加 `default-run`**，那是行为变更。
- 不加 `rust-toolchain.toml` / `rustfmt.toml`，格式与 lint 全用默认规则。
- 依赖只用最新版、禁止降级；升级 = `cargo update` + 全量重编（第一次很慢，正常）。

## gpui-kit

- GUI 依赖只有 `gpui-kit = "0.6"`。代码里**只允许 `use gpui_kit::...`**（组件在 `gpui_kit::component::`），全仓直接引用 `gpui::` / `gpui_component::` 为 0，别破坏这个不变量，也别往 Cargo.toml 加那两个依赖。
- Cargo.toml 里 `[profile.dev.package.gpui-component] debug-assertions = false` **不能删**：组件库 0.6.4 的 inspector 按 gpui-pre 0.3.5 签名写、与 0.3.6 不兼容，该模块只在 debug_assertions 下编译。遇 gpui-component 编译错误先怀疑这一条被动过，不要升降级版本绕过去。

## GPUI 布局硬坑（真实回归的根因，别凭 web 直觉写）

- `div()` 默认块级；`.flex_1()` / `.flex_row()` **不会**把 display 改成 flex。要 flex 容器必须显式 `.flex()`。
- 块级父节点下 `size_full()` / `h_full()` 塌成 0。表格必须是 `flex_1().min_h_0()` 容器的**直接子节点**，空档用 `pr`/`pb` 留白，别加层。
- 新表一律走 `table_scroll_area()`（`src/ui/mod.rs`），不要各自手搓滚动条。
- 尺寸一律 rem（`ui_rem(cx)`），不写死 px，除非该尺寸本身是常量语义。
- `ui::init(cx)` 必须在 `cx.open_window(...)` **之前**，反了菜单和快捷键不生效。
- 快捷键带上下文 `CONTEXT = "LocTool"`，只有主视图响应（浮层窗口故意不接）。加键要同步改：动作、键位、应用菜单、README 快捷键表四处。

## 运行时行为

- 扫描缓存是进程级全局 static（`src/loc/scanner.rs`），测试间共享；写用例别假设每次都真扫。
- 取消扫描后 `scan_directory*` 返回的是**不完整的部分结果且 `Ok(..)`**，调用方必须自己区分，别把部分结果当总计展示；这条路径不写缓存。
- 默认排除项唯一来源是 `AppConfig::default()`（`src/config.rs`），scanner 里没有内置清单；改动默认值时同步 README「默认排除项」。
- 过滤语义有个不对称，且代码注释与实现不一致：`starts_with('.')` 挂在 `filter_entry` 上，**目录和文件都会跳过**（注释只写了 "Skip hidden directories"）；排除目录是目录名精确匹配、**区分大小写**，排除文件是文件名通配、**不区分大小写**；两者都只匹配名字、不含路径（`src/loc/scanner.rs`）。
- 配置在 `dirs::config_dir()/cc_loc_tool/config.toml`，设置**改动即时落盘**。新增配置字段要带 `#[serde(default)]`。
- 语言/扩展名定义只在 `src/language.rs` 一处，加语言改这里；自定义后缀解析统一 `trim().trim_start_matches('.').to_lowercase()`。
- 圈复杂度是文本近似不是真解析器，默认关闭且不要偷偷默认开。

## 仓库约定

- `git add` 报 "LF would be replaced by CRLF" 被拒时：按该文件的行尾约定重写行尾，别加 `-f`、别改 git 配置。
- 提交信息用中文约定式提交（`feat(ui): …` / `fix: …` / `docs` / `chore`）。
- **不要对 `main` 做 force-push**；要重写历史先 `git bundle` 备份 + 推备份分支，force-push 命令交给我自己执行。

## 工具与编辑习惯

- 找文件用 `fd` 或 `rg --files`，找内容用 `rg`；自动化里不启动交互式 `fzf`，模糊筛选走 `fzf --filter`。
- 编辑时保留仓库现有编码与行尾；`.gitattributes` 覆盖的后缀（rs/toml/md/yml）新文件按 LF、UTF-8 无 BOM。
- 不绕过 `.gitignore` 用 `git add -f`，除非用户点名具体路径。
- `pub` API 按 Rust 惯例写文档注释；文件头注释只在用途从路径看不出来时才加；错误处理只写在真实的边界上。

## UI 工作与交付

- 做任何界面前先读 `gpui-kit`（组件真实 API，API 与直觉不符时以它为准）和 `gpui-kit-design-guides`（规范性：token 优先不许裸 hex、hover/focus/disabled 状态必须可见、`Esc` 关最上层浮层、文案点名对象和动作）两个技能，收尾跑一遍后者里的 Design 与 Accessibility checklist。拿不到这两个技能时，以 crates.io 上 `gpui-kit` 对应版本的文档和组件库源码为准，不要凭训练直觉写组件 API。
- 界面改动**不要自己截图/注入点击去测**；需要人肉验证时把步骤告诉我。但上面三条门禁必须自己跑并报告真实输出。
- 报完成前对抗性自查自己的 diff：边界时序（扫描中改设置、竞态）、放大效应。**如实列出没验证到的地方**。
- 界面改动同步更新 README 与 ROADMAP；明显变化重截 `assets/screenshot.png`。
- 保持改动范围最小：不顺手加测试目标、配置项、抽象或"顺便重构"。三处重复好过提前抽象。
