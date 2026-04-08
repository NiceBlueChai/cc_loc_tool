use anyhow::Result;
use encoding_rs::{GBK, UTF_8};
use serde::Serialize;
use std::{fs, path::PathBuf};

use crate::complexity::{ComplexitySummary, FileComplexity, analyze_file_complexity};
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
        let mut total_function_lines = 0usize;
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
                total_function_lines += c.functions.iter().map(|func| func.lines).sum::<usize>();
                high_complexity += c.high_complexity_count();
                long_functions += c.long_function_count();
            }
        }

        if total_functions > 0 {
            summary.complexity = Some(ComplexitySummary {
                total_cyclomatic,
                avg_cyclomatic: total_cyclomatic as f64 / total_functions as f64,
                total_functions,
                avg_function_length: total_function_lines as f64 / total_functions as f64,
                high_complexity_functions: high_complexity,
                long_functions,
            });
        }

        summary
    }
}

fn count_lines_by_language(content: &str, language: Language) -> (usize, usize, usize) {
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

            if let Some(delimiter) = python_multiline_delimiter_for_line(trimmed) {
                comments += 1;
                if !trimmed.ends_with(delimiter) || trimmed == delimiter {
                    in_python_multiline_comment = true;
                    python_multiline_delimiter = delimiter;
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

    (code, comments, blanks)
}

fn python_multiline_delimiter_for_line(trimmed: &str) -> Option<&'static str> {
    if trimmed.starts_with("'''") {
        Some("'''")
    } else if trimmed.starts_with("\"\"\"") {
        Some("\"\"\"")
    } else {
        None
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
    let (code, comments, blanks) = count_lines_by_language(&content, language);

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
    let (code, comments, blanks) = count_lines_by_language(&content, language);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complexity::{FileComplexity, FunctionStats};

    #[test]
    fn python_triple_double_quotes_are_counted_as_comments() {
        let content = r#"
"""
module doc
"""
print("hello")
"#;

        let (code, comments, blanks) = count_lines_by_language(content, Language::Python);

        assert_eq!(code, 1);
        assert_eq!(comments, 3);
        assert_eq!(blanks, 1);
    }

    #[test]
    fn summary_with_complexity_computes_average_function_length() {
        let files = vec![FileLoc {
            path: PathBuf::from("example.rs"),
            code: 10,
            comments: 2,
            blanks: 1,
            complexity: Some(FileComplexity {
                path: PathBuf::from("example.rs"),
                cyclomatic: 7,
                avg_cyclomatic: 3.5,
                max_cyclomatic: 4,
                functions: vec![
                    FunctionStats {
                        name: "a".into(),
                        start_line: 1,
                        end_line: 3,
                        lines: 3,
                        cyclomatic: 3,
                        parameter_count: 0,
                    },
                    FunctionStats {
                        name: "b".into(),
                        start_line: 5,
                        end_line: 10,
                        lines: 6,
                        cyclomatic: 4,
                        parameter_count: 1,
                    },
                ],
                avg_function_length: 4.5,
                max_function_length: 6,
            }),
        }];

        let summary = LocSummary::from_files_with_complexity(&files);

        assert_eq!(
            summary.complexity.as_ref().map(|c| c.avg_function_length),
            Some(4.5)
        );
    }
}
