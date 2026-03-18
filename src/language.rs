/// 支持的编程语言
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Language {
    C,
    Cpp,
    Java,
    Python,
    Go,
    Rust,
}

impl Language {
    /// 获取所有支持的语言
    pub fn all() -> &'static [Self] {
        &[
            Self::C,
            Self::Cpp,
            Self::Java,
            Self::Python,
            Self::Go,
            Self::Rust,
        ]
    }

    /// 获取语言的显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::C => "C",
            Self::Cpp => "C++",
            Self::Java => "Java",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::Rust => "Rust",
        }
    }

    /// 获取语言的文件扩展名
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::C => &["c", "h"],
            Self::Cpp => &["cc", "cpp", "cxx", "h", "hpp", "hxx", "inl"],
            Self::Java => &["java"],
            Self::Python => &["py"],
            Self::Go => &["go"],
            Self::Rust => &["rs"],
        }
    }

    /// 检查文件是否匹配此语言的扩展名
    pub fn matches_file(&self, path: &std::path::Path) -> bool {
        let Some(ext) = path.extension() else {
            return false;
        };
        let ext = ext.to_string_lossy().to_lowercase();
        self.extensions().contains(&ext.as_str())
    }
}

/// 检查文件扩展名是否被任何支持的语言匹配
pub fn is_supported_file(path: &std::path::Path, languages: &[Language]) -> bool {
    languages.iter().any(|lang| lang.matches_file(path))
}
