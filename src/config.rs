use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::language::Language;
use crate::ui::Theme;

/// 应用程序配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 最后选择的目录路径
    pub last_selected_path: Option<PathBuf>,

    /// 排除的目录列表
    pub exclude_dirs: HashSet<String>,

    /// 排除的文件模式列表
    pub exclude_files: Vec<String>,

    /// 选中的编程语言列表
    pub selected_languages: Vec<String>,

    /// 应用程序主题
    pub theme: Theme,

    /// 是否启用复杂度分析
    #[serde(default)]
    pub analyze_complexity: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            last_selected_path: None,
            exclude_dirs: HashSet::from([
                "node_modules".to_string(),
                ".git".to_string(),
                "target".to_string(),
            ]),
            exclude_files: vec![
                "*.generated.*".to_string(),
                "moc_*".to_string(),
                "qrc_*".to_string(),
            ],
            selected_languages: Language::all()
                .iter()
                .map(|l| l.display_name().to_string())
                .collect(),
            theme: Theme::Light,
            analyze_complexity: false, // 默认关闭以提升扫描速度
        }
    }
}

impl AppConfig {
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;

        let app_config_dir = config_dir.join("cc_loc_tool");
        fs::create_dir_all(&app_config_dir)?;

        Ok(app_config_dir.join("config.toml"))
    }

    /// 从配置文件加载配置
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let mut config: Self = toml::from_str(&content)?;

            // 确保语言列表是有效的
            config
                .selected_languages
                .retain(|lang| Language::all().iter().any(|l| l.display_name() == lang));

            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// 保存配置到文件
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        let content = toml::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }

    /// 获取选中的语言枚举列表
    pub fn get_selected_languages(&self) -> Vec<Language> {
        if self.selected_languages.is_empty() {
            return Language::all().to_vec();
        }

        self.selected_languages
            .iter()
            .filter_map(|lang_name| {
                Language::all()
                    .iter()
                    .find(|l| l.display_name() == lang_name)
                    .copied()
            })
            .collect()
    }

    /// 设置选中的语言枚举列表
    pub fn set_selected_languages(&mut self, languages: &[Language]) {
        self.selected_languages = languages
            .iter()
            .map(|lang| lang.display_name().to_string())
            .collect();
    }

    /// 将排除目录转换为字符串（用于UI显示）
    pub fn exclude_dirs_to_string(&self) -> String {
        self.exclude_dirs
            .iter()
            .cloned()
            .collect::<Vec<String>>()
            .join(", ")
    }

    /// 将字符串解析为排除目录集合
    pub fn exclude_dirs_from_string(&mut self, s: &str) {
        self.exclude_dirs = s
            .split(|c| c == ',' || c == ';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }

    /// 将排除文件转换为字符串（用于UI显示）
    pub fn exclude_files_to_string(&self) -> String {
        self.exclude_files
            .iter()
            .cloned()
            .collect::<Vec<String>>()
            .join(", ")
    }

    /// 将字符串解析为排除文件列表
    pub fn exclude_files_from_string(&mut self, s: &str) {
        self.exclude_files = s
            .split(|c| c == ',' || c == ';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
}
