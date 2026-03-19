use anyhow::Result;
use encoding_rs::{GBK, UTF_8};
use serde::Serialize;
use std::{fs, path::PathBuf};

use crate::complexity::{analyze_file_complexity, ComplexitySummary, FileComplexity};
use crate::language::Language;

/// 超过此大小的文件将跳过复杂度分析（单位：字节）
/// 默认 1MB = 1024 * 1024 = 1048576 字节
const LARGE_FILE_THRESHOLD: u64 = 1024 * 1024;

/// Statistics for a single file
#[derive(Clone, Debug, Serialize)]
pub struct FileLoc {
    pub path: PathBuf,
    pub code: usize,
    pub comments: usize,
    pub blanks: usize,
    /// 复杂度分析结果（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity: Option<FileComplexity>,
}

impl FileLoc {
    pub fn total(&self) -> usize {
        self.code + self.comments + self.blanks
    }
}

/// Aggregate statistics
#[derive(Clone, Debug, Default, Serialize)]
pub struct LocSummary {
    pub files: usize,
    pub code: usize,
    pub comments: usize,
    pub blanks: usize,
    /// 复杂度汇总（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity: Option<ComplexitySummary>,
}

impl LocSummary {
    pub fn total(&self) -> usize {
        self.code + self.comments + self.blanks
    }

    pub fn from_files(files: &[FileLoc]) -> Self {
        let mut summary = Self::default();
        summary.files = files.len();
        for f in files {
            summary.code += f.code;
            summary.comments += f.comments;
            summary.blanks += f.blanks;
        }
        summary
    }
    
    /// 从文件列表计算汇总，包含复杂度统计
    pub fn from_files_with_complexity(files: &[FileLoc]) -> Self {
        let mut summary = Self::default();
        summary.files = files.len();
        for f in files {
            summary.code += f.code;
            summary.comments += f.comments;
            summary.blanks += f.blanks;
        }
        
        // 计算复杂度汇总（优化：避免不必要的clone）
        let mut total_cyclomatic = 0usize;
        let mut total_functions = 0usize;
        let mut high_complexity = 0usize;
        let mut long_functions = 0usize;
        
        for f in files {
            if let Some(ref c) = f.complexity {
                total_cyclomatic += c.cyclomatic;
                total_functions += c.functions.len();
                high_complexity += c.high_complexity_count();
                long_functions += c.long_function_count();
            }
        }
        
        if total_functions > 0 {
            summary.complexity = Some(ComplexitySummary {
                total_cyclomatic,
                avg_cyclomatic: total_cyclomatic as f64 / total_functions as f64,
                total_functions,
                avg_function_length: 0.0, // 简化计算
                high_complexity_functions: high_complexity,
                long_functions,
            });
        }
        
        summary
    }
}

/// Detect language from file extension
fn detect_language(path: &std::path::Path) -> Option<Language> {
    Language::all()
        .iter()
        .find(|&&lang| lang.matches_file(path))
        .copied()
}

/// Read file content with auto-detection of encoding (UTF-8 or GBK)
pub fn read_file_content(path: &std::path::Path) -> Result<String> {
    let bytes = fs::read(path)?;

    // Try UTF-8 first
    let (cow, _, had_errors) = UTF_8.decode(&bytes);
    if !had_errors {
        return Ok(cow.into_owned());
    }

    // Fallback to GBK (common for Chinese Windows)
    let (cow, _, _) = GBK.decode(&bytes);
    Ok(cow.into_owned())
}

/// Count lines in a single file
pub fn count_file(path: &std::path::Path) -> Result<FileLoc> {
    let content = read_file_content(path)?;
    let language = detect_language(path).unwrap_or(Language::Cpp); // Default to C++ if unknown

    let mut code = 0usize;
    let mut comments = 0usize;
    let mut blanks = 0usize;
    let mut in_block_comment = false;
    let mut in_python_multiline_comment = false;
    let mut python_multiline_delimiter = "";

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            blanks += 1;
            continue;
        }

        // Handle Python multiline comments
        if language == Language::Python {
            if in_python_multiline_comment {
                comments += 1;
                if trimmed.ends_with(python_multiline_delimiter) {
                    in_python_multiline_comment = false;
                    python_multiline_delimiter = "";
                }
                continue;
            }

            // Check for multiline comment start
            if trimmed.starts_with("'''") {
                comments += 1;
                if !trimmed.ends_with("'''") || trimmed == "'''" {
                    in_python_multiline_comment = true;
                    python_multiline_delimiter = "'''";
                }
                continue;
            } else if trimmed.starts_with("\'\'\'") {
                comments += 1;
                if !trimmed.ends_with("\'\'\'") || trimmed == "\'\'\'" {
                    in_python_multiline_comment = true;
                    python_multiline_delimiter = "\'\'\'";
                }
                continue;
            }
        }

        // Handle C-style block comments (C, C++, Java, Rust, Go)
        if in_block_comment {
            comments += 1;
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        // Check for line comments based on language
        match language {
            Language::Python => {
                if trimmed.starts_with('#') {
                    comments += 1;
                } else {
                    code += 1;
                }
            }
            _ => {
                // C-style languages (C, C++, Java, Rust, Go)
                if trimmed.starts_with("//") {
                    comments += 1;
                } else if trimmed.starts_with("/*") {
                    comments += 1;
                    if !trimmed.contains("*/") {
                        in_block_comment = true;
                    }
                } else {
                    code += 1;
                    // Handle inline block comment start
                    if trimmed.contains("/*") && !trimmed.contains("*/") {
                        in_block_comment = true;
                    }
                }
            }
        }
    }

    Ok(FileLoc {
        path: path.to_path_buf(),
        code,
        comments,
        blanks,
        complexity: None,
    })
}

/// Count lines and analyze complexity in a single file
pub fn count_file_with_complexity(path: &std::path::Path) -> Result<FileLoc> {
    let content = read_file_content(path)?;
    let language = detect_language(path).unwrap_or(Language::Cpp);

    let mut code = 0usize;
    let mut comments = 0usize;
    let mut blanks = 0usize;
    let mut in_block_comment = false;
    let mut in_python_multiline_comment = false;
    let mut python_multiline_delimiter = "";

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            blanks += 1;
            continue;
        }

        if language == Language::Python {
            if in_python_multiline_comment {
                comments += 1;
                if trimmed.ends_with(python_multiline_delimiter) {
                    in_python_multiline_comment = false;
                    python_multiline_delimiter = "";
                }
                continue;
            }

            if trimmed.starts_with("'''") {
                comments += 1;
                if !trimmed.ends_with("'''") || trimmed == "'''" {
                    in_python_multiline_comment = true;
                    python_multiline_delimiter = "'''";
                }
                continue;
            } else if trimmed.starts_with("\'\'\'") {
                comments += 1;
                if !trimmed.ends_with("\'\'\'") || trimmed == "\'\'\'" {
                    in_python_multiline_comment = true;
                    python_multiline_delimiter = "\'\'\'";
                }
                continue;
            }
        }

        if in_block_comment {
            comments += 1;
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        match language {
            Language::Python => {
                if trimmed.starts_with('#') {
                    comments += 1;
                } else {
                    code += 1;
                }
            }
            _ => {
                if trimmed.starts_with("//") {
                    comments += 1;
                } else if trimmed.starts_with("/*") {
                    comments += 1;
                    if !trimmed.contains("*/") {
                        in_block_comment = true;
                    }
                } else {
                    code += 1;
                    if trimmed.contains("/*") && !trimmed.contains("*/") {
                        in_block_comment = true;
                    }
                }
            }
        }
    }

    // 分析复杂度（对于大文件跳过以节省内存）
    let file_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let complexity = if file_size > LARGE_FILE_THRESHOLD {
        // 大文件跳过复杂度分析
        None
    } else {
        analyze_file_complexity(&content, &path.to_path_buf(), language)
    };

    Ok(FileLoc {
        path: path.to_path_buf(),
        code,
        comments,
        blanks,
        complexity,
    })
}
