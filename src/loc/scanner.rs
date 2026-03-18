use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashSet;
use walkdir::WalkDir;

use super::counter::{FileLoc, count_file};

/// Supported programming languages
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
    /// Get all supported languages
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

    /// Get display name for the language
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

    /// Get file extensions for the language
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

    /// Check if the file has an extension associated with this language
    pub fn matches_file(&self, path: &std::path::Path) -> bool {
        let Some(ext) = path.extension() else {
            return false;
        };
        let ext = ext.to_string_lossy().to_lowercase();
        self.extensions().contains(&ext.as_str())
    }
}

/// Check if the file extension is supported by any language
pub fn is_supported_file(path: &std::path::Path, languages: &[Language]) -> bool {
    languages.iter().any(|lang| lang.matches_file(path))
}

/// Simple wildcard pattern matching (* matches any characters)
fn matches_pattern(pattern: &str, text: &str) -> bool {
    let pattern = pattern.to_lowercase();
    let text = text.to_lowercase();

    if !pattern.contains('*') {
        // No wildcard, exact match
        return pattern == text;
    }

    let parts: Vec<&str> = pattern.split('*').collect();

    if parts.len() == 2 {
        // Simple cases: *.ext, prefix*, *middle*
        let (prefix, suffix) = (parts[0], parts[1]);
        if prefix.is_empty() && suffix.is_empty() {
            return true; // Pattern is just "*"
        }
        if prefix.is_empty() {
            return text.ends_with(suffix); // *.ext
        }
        if suffix.is_empty() {
            return text.starts_with(prefix); // prefix*
        }
        return text.starts_with(prefix) && text.ends_with(suffix); // prefix*suffix
    }

    // Complex pattern with multiple wildcards
    let mut text_pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if let Some(pos) = text[text_pos..].find(*part) {
            if i == 0 && pos != 0 {
                return false; // First part must match at start
            }
            text_pos += pos + part.len();
        } else {
            return false;
        }
    }

    // If pattern ends with *, any remaining text is ok
    // If pattern doesn't end with *, text must be exhausted
    parts.last().map(|p| p.is_empty()).unwrap_or(true) || text_pos == text.len()
}

/// Walk directory and count all supported files
pub fn scan_directory(
    root: &std::path::Path,
    exclude_dirs: &HashSet<String>,
    exclude_files: &[String],
    languages: &[Language],
    progress_callback: Option<&dyn Fn(usize, usize)>,
) -> Result<Vec<FileLoc>> {
    // 收集所有符合条件的文件路径
    let files: Vec<_> = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Skip hidden directories
            if name.starts_with('.') {
                return false;
            }
            // Skip excluded directories
            if e.file_type().is_dir() && exclude_dirs.contains(name.as_ref()) {
                return false;
            }
            true
        })
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return false;
            }

            // 检查是否为支持的文件类型
            if !is_supported_file(path, languages) {
                return false;
            }

            // 检查是否需要排除
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            !exclude_files
                .iter()
                .any(|pattern| matches_pattern(pattern, &file_name))
        })
        .map(|entry| entry.path().to_path_buf())
        .collect();

    let total_files = files.len();

    // 如果有进度回调，先报告0%进度
    if let Some(callback) = progress_callback {
        callback(0, total_files);
    }

    // 并行处理所有文件
    let results: Vec<FileLoc> = files
        .par_iter() // 并行迭代器
        .filter_map(|path| match count_file(path) {
            Ok(loc) => Some(loc),
            Err(e) => {
                eprintln!("Error reading {:?}: {}", path, e);
                None
            }
        })
        .collect(); // 收集结果

    // 报告100%进度
    if let Some(callback) = progress_callback {
        callback(total_files, total_files);
    }

    Ok(results)
}

/// Walk directory and count all supported files without progress callback
pub fn scan_directory_simple(
    root: &std::path::Path,
    exclude_dirs: &HashSet<String>,
    exclude_files: &[String],
    languages: &[Language],
) -> Result<Vec<FileLoc>> {
    scan_directory(root, exclude_dirs, exclude_files, languages, None)
}
