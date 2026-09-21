use gpui_kit::component::GlobalState;
use gpui_kit::{App, KeyBinding, Menu, MenuItem};

/// 主窗口元素的按键上下文。只有带这个上下文的元素（主视图）会响应下面的快捷键，
/// 预览窗口、复杂度详情窗口互不干扰。
pub const CONTEXT: &str = "LocTool";

gpui_kit::actions!(
    loc_tool,
    [
        BrowseProject,
        ScanProject,
        CancelScan,
        ExportResults,
        SaveSnapshot,
        CompareSnapshot,
        ToggleTheme,
        OpenSelectedFile,
        ShowSelectedComplexity,
        CopySelectedPath,
        SelectAllLanguages,
        ClearLanguages,
        DismissNotice,
    ]
);

/// 注册快捷键。`secondary` 在 macOS 上是 Command，其他平台是 Control。
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-o", BrowseProject, Some(CONTEXT)),
        KeyBinding::new("secondary-r", ScanProject, Some(CONTEXT)),
        KeyBinding::new("escape", CancelScan, Some(CONTEXT)),
        KeyBinding::new("secondary-e", ExportResults, Some(CONTEXT)),
        KeyBinding::new("secondary-s", SaveSnapshot, Some(CONTEXT)),
        KeyBinding::new("secondary-shift-s", CompareSnapshot, Some(CONTEXT)),
        KeyBinding::new("secondary-d", ToggleTheme, Some(CONTEXT)),
        KeyBinding::new("enter", OpenSelectedFile, Some(CONTEXT)),
        KeyBinding::new("secondary-i", ShowSelectedComplexity, Some(CONTEXT)),
        KeyBinding::new("secondary-c", CopySelectedPath, Some(CONTEXT)),
    ]);
}

/// 安装应用菜单。必须在创建 `AppMenuBar` 之前调用。
pub fn install_app_menus(cx: &mut App) {
    let menus = vec![
        Menu::new("文件")
            .items([
                MenuItem::action("选择项目目录…", BrowseProject),
                MenuItem::action("导出结果…", ExportResults),
            ])
            .owned(),
        Menu::new("扫描")
            .items([
                MenuItem::action("开始扫描", ScanProject),
                MenuItem::action("取消扫描", CancelScan),
            ])
            .owned(),
        Menu::new("结果")
            .items([
                MenuItem::action("打开文件", OpenSelectedFile),
                MenuItem::action("复杂度详情", ShowSelectedComplexity),
                MenuItem::separator(),
                MenuItem::action("复制路径", CopySelectedPath),
                MenuItem::separator(),
                MenuItem::action("保存快照…", SaveSnapshot),
                MenuItem::action("历史对比…", CompareSnapshot),
            ])
            .owned(),
        Menu::new("视图")
            .items([
                MenuItem::action("切换主题", ToggleTheme),
                MenuItem::separator(),
                MenuItem::action("全选语言", SelectAllLanguages),
                MenuItem::action("清空语言", ClearLanguages),
            ])
            .owned(),
    ];

    GlobalState::global_mut(cx).set_app_menus(menus);
}
