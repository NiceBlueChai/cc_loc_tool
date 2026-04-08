use anyhow::Result;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

use super::counter::{FileLoc, count_file, count_file_with_complexity};
use crate::language::{Language, is_supported_file_with_custom};

type ProgressCallback = dyn Fn(usize, usize) + Sync;
const MAX_CACHE_ENTRIES: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ScanCacheKey {
    root: String,
    exclude_dirs: Vec<String>,
    exclude_files: Vec<String>,
    languages: Vec<String>,
    custom_extensions: Vec<String>,
    analyze_complexity: bool,
}

#[derive(Clone)]
struct ScanCacheEntry {
    signature: u64,
    results: Vec<FileLoc>,
    last_access_tick: u64,
}

static SCAN_CACHE: OnceLock<Mutex<HashMap<ScanCacheKey, ScanCacheEntry>>> = OnceLock::new();
static CACHE_TICK: AtomicU64 = AtomicU64::new(1);
#[cfg(test)]
static CACHE_HITS: AtomicUsize = AtomicUsize::new(0);

fn cache_store() -> &'static Mutex<HashMap<ScanCacheKey, ScanCacheEntry>> {
    SCAN_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_cache_tick() -> u64 {
    CACHE_TICK.fetch_add(1, Ordering::Relaxed)
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
    root: &Path,
    exclude_dirs: &HashSet<String>,
    exclude_files: &[String],
    languages: &[Language],
    custom_extensions: &[String],
    progress_callback: Option<&ProgressCallback>,
) -> Result<Vec<FileLoc>> {
    scan_directory_internal(
        root,
        exclude_dirs,
        exclude_files,
        languages,
        custom_extensions,
        progress_callback,
        false,
    )
}

fn build_cache_key(
    root: &Path,
    exclude_dirs: &HashSet<String>,
    exclude_files: &[String],
    languages: &[Language],
    custom_extensions: &[String],
    analyze_complexity: bool,
) -> ScanCacheKey {
    let root = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .to_string();

    let mut exclude_dirs: Vec<String> = exclude_dirs.iter().map(|s| s.to_lowercase()).collect();
    exclude_dirs.sort();

    let mut exclude_files: Vec<String> = exclude_files.iter().map(|s| s.to_lowercase()).collect();
    exclude_files.sort();

    let mut languages: Vec<String> = languages
        .iter()
        .map(|language| language.display_name().to_lowercase())
        .collect();
    languages.sort();

    let mut custom_extensions: Vec<String> = custom_extensions
        .iter()
        .map(|s| s.trim().trim_start_matches('.').to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    custom_extensions.sort();

    ScanCacheKey {
        root,
        exclude_dirs,
        exclude_files,
        languages,
        custom_extensions,
        analyze_complexity,
    }
}

fn collect_files(
    root: &Path,
    exclude_dirs: &HashSet<String>,
    exclude_files: &[String],
    languages: &[Language],
    custom_extensions: &[String],
) -> Vec<std::path::PathBuf> {
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
            if !is_supported_file_with_custom(path, languages, custom_extensions) {
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
    files
}

fn compute_files_signature(files: &[std::path::PathBuf]) -> Option<u64> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    for path in files {
        path.hash(&mut hasher);

        let metadata = fs::metadata(path).ok()?;
        metadata.len().hash(&mut hasher);

        let modified = metadata.modified().ok()?;
        let timestamp_ns = modified.duration_since(UNIX_EPOCH).ok()?.as_nanos();
        timestamp_ns.hash(&mut hasher);
    }

    Some(hasher.finish())
}

fn try_get_cached_result(cache_key: &ScanCacheKey, signature: u64) -> Option<Vec<FileLoc>> {
    let mut cache = cache_store().lock().ok()?;
    let entry = cache.get_mut(cache_key)?;
    if entry.signature != signature {
        return None;
    }

    entry.last_access_tick = next_cache_tick();
    #[cfg(test)]
    CACHE_HITS.fetch_add(1, Ordering::Relaxed);
    Some(entry.results.clone())
}

fn put_cached_result(cache_key: ScanCacheKey, signature: u64, results: Vec<FileLoc>) {
    let Ok(mut cache) = cache_store().lock() else {
        return;
    };

    let entry = ScanCacheEntry {
        signature,
        results: results.clone(),
        last_access_tick: next_cache_tick(),
    };
    cache.insert(cache_key, entry);

    while cache.len() > MAX_CACHE_ENTRIES {
        if let Some(evict_key) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_access_tick)
            .map(|(key, _)| key.clone())
        {
            cache.remove(&evict_key);
        } else {
            break;
        }
    }
}

fn scan_directory_internal(
    root: &Path,
    exclude_dirs: &HashSet<String>,
    exclude_files: &[String],
    languages: &[Language],
    custom_extensions: &[String],
    progress_callback: Option<&ProgressCallback>,
    analyze_complexity: bool,
) -> Result<Vec<FileLoc>> {
    // 收集所有符合条件的文件路径（使用引用避免复制）
    let files = collect_files(
        root,
        exclude_dirs,
        exclude_files,
        languages,
        custom_extensions,
    );

    let total_files = files.len();

    let cache_key = build_cache_key(
        root,
        exclude_dirs,
        exclude_files,
        languages,
        custom_extensions,
        analyze_complexity,
    );
    let signature = compute_files_signature(&files);

    // 如果有进度回调，先报告0%进度
    if let Some(callback) = progress_callback {
        callback(0, total_files);
    }

    if let Some(signature) = signature
        && let Some(cached) = try_get_cached_result(&cache_key, signature)
    {
        if let Some(callback) = progress_callback {
            callback(total_files, total_files);
        }
        return Ok(cached);
    }

    let processed = AtomicUsize::new(0);

    // 并行处理所有文件
    let results: Vec<FileLoc> = files
        .par_iter() // 并行迭代器
        .filter_map(|path| {
            let result = match if analyze_complexity {
                count_file_with_complexity(path)
            } else {
                count_file(path)
            } {
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

    if let Some(signature) = signature {
        put_cached_result(cache_key, signature, results.clone());
    }

    Ok(results)
}

/// Walk directory and count all supported files without progress callback
pub fn scan_directory_simple(
    root: &std::path::Path,
    exclude_dirs: &HashSet<String>,
    exclude_files: &[String],
    languages: &[Language],
    custom_extensions: &[String],
) -> Result<Vec<FileLoc>> {
    scan_directory(
        root,
        exclude_dirs,
        exclude_files,
        languages,
        custom_extensions,
        None,
    )
}

/// Walk directory and count all supported files with complexity analysis
pub fn scan_directory_with_complexity(
    root: &Path,
    exclude_dirs: &HashSet<String>,
    exclude_files: &[String],
    languages: &[Language],
    custom_extensions: &[String],
    progress_callback: Option<&ProgressCallback>,
) -> Result<Vec<FileLoc>> {
    scan_directory_internal(
        root,
        exclude_dirs,
        exclude_files,
        languages,
        custom_extensions,
        progress_callback,
        true,
    )
}

#[cfg(test)]
fn reset_cache_for_tests() {
    if let Ok(mut cache) = cache_store().lock() {
        cache.clear();
    }
    CACHE_HITS.store(0, Ordering::Relaxed);
}

#[cfg(test)]
fn cache_hits_for_tests() -> usize {
    CACHE_HITS.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir() -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cc_loc_tool_cache_test_{}", timestamp));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn repeated_scan_uses_cache_and_file_change_invalidates_cache() {
        reset_cache_for_tests();

        let root = make_temp_dir();
        let file_path = root.join("main.cpp");

        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "int main() {{").unwrap();
        writeln!(file, "    return 0;").unwrap();
        writeln!(file, "}}").unwrap();

        let exclude_dirs = HashSet::new();
        let exclude_files: Vec<String> = Vec::new();
        let languages = vec![Language::Cpp];
        let custom_extensions: Vec<String> = Vec::new();

        let first = scan_directory_simple(
            &root,
            &exclude_dirs,
            &exclude_files,
            &languages,
            &custom_extensions,
        )
        .unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(cache_hits_for_tests(), 0);

        let second = scan_directory_simple(
            &root,
            &exclude_dirs,
            &exclude_files,
            &languages,
            &custom_extensions,
        )
        .unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(cache_hits_for_tests(), 1);

        fs::write(
            &file_path,
            "int main() {\n    return 0;\n}\nint value = 1;\n",
        )
        .unwrap();

        let third = scan_directory_simple(
            &root,
            &exclude_dirs,
            &exclude_files,
            &languages,
            &custom_extensions,
        )
        .unwrap();
        assert_eq!(third.len(), 1);
        assert_eq!(third[0].code, 4);
        assert_eq!(cache_hits_for_tests(), 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn custom_extension_is_scanned() {
        reset_cache_for_tests();

        let root = make_temp_dir();
        let file_path = root.join("custom.tpp");
        fs::write(&file_path, "int main() {\n    return 0;\n}\n").unwrap();

        let exclude_dirs = HashSet::new();
        let exclude_files: Vec<String> = Vec::new();
        let languages: Vec<Language> = Vec::new();
        let custom_extensions = vec!["tpp".to_string()];

        let results = scan_directory_simple(
            &root,
            &exclude_dirs,
            &exclude_files,
            &languages,
            &custom_extensions,
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].code, 3);

        let _ = fs::remove_dir_all(root);
    }
}
