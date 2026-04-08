use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

use super::counter::{FileLoc, count_file, count_file_with_complexity};
use crate::language::{Language, is_supported_file};

type ProgressCallback = dyn Fn(usize, usize) + Sync;

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
    root: &Path,
    exclude_dirs: &HashSet<String>,
    exclude_files: &[String],
    languages: &[Language],
    progress_callback: Option<&ProgressCallback>,
) -> Result<Vec<FileLoc>> {
    // 收集所有符合条件的文件路径（使用引用避免复制）
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

            // 检查是否需要排除（优化：直接使用OsStr比较，避免创建String）
            if let Some(file_name) = path.file_name() {
                let file_name_str = file_name.to_string_lossy();
                !exclude_files
                    .iter()
                    .any(|pattern| matches_pattern(pattern, &file_name_str))
            } else {
                false
            }
        })
        .map(|entry| entry.path().to_path_buf())
        .collect();

    let total_files = files.len();

    // 如果有进度回调，先报告0%进度
    if let Some(callback) = progress_callback {
        callback(0, total_files);
    }

    let processed = AtomicUsize::new(0);

    // 并行处理所有文件
    let results: Vec<FileLoc> = files
        .par_iter() // 并行迭代器
        .filter_map(|path| {
            let result = match count_file(path) {
                Ok(loc) => Some(loc),
                Err(e) => {
                    eprintln!("Error reading {:?}: {}", path, e);
                    None
                }
            };

            if let Some(callback) = progress_callback {
                let current = processed.fetch_add(1, Ordering::Relaxed) + 1;
                callback(current, total_files);
            }

            result
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

/// Walk directory and count all supported files with complexity analysis
pub fn scan_directory_with_complexity(
    root: &Path,
    exclude_dirs: &HashSet<String>,
    exclude_files: &[String],
    languages: &[Language],
    progress_callback: Option<&ProgressCallback>,
) -> Result<Vec<FileLoc>> {
    // 收集所有符合条件的文件路径
    let files: Vec<_> = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            if name.starts_with('.') {
                return false;
            }
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

            if !is_supported_file(path, languages) {
                return false;
            }

            // 优化：直接使用OsStr比较，避免创建String
            if let Some(file_name) = path.file_name() {
                let file_name_str = file_name.to_string_lossy();
                !exclude_files
                    .iter()
                    .any(|pattern| matches_pattern(pattern, &file_name_str))
            } else {
                false
            }
        })
        .map(|entry| entry.path().to_path_buf())
        .collect();

    let total_files = files.len();

    if let Some(callback) = progress_callback {
        callback(0, total_files);
    }

    let processed = AtomicUsize::new(0);

    // 并行处理所有文件，包含复杂度分析
    let results: Vec<FileLoc> = files
        .par_iter()
        .filter_map(|path| {
            let result = match count_file_with_complexity(path) {
                Ok(loc) => Some(loc),
                Err(e) => {
                    eprintln!("Error reading {:?}: {}", path, e);
                    None
                }
            };

            if let Some(callback) = progress_callback {
                let current = processed.fetch_add(1, Ordering::Relaxed) + 1;
                callback(current, total_files);
            }

            result
        })
        .collect();

    if let Some(callback) = progress_callback {
        callback(total_files, total_files);
    }

    Ok(results)
}
