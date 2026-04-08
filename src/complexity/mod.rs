mod cyclomatic;
mod function_stats;
mod metrics;

pub use cyclomatic::{CyclomaticStats, calculate_cyclomatic_complexity};
pub use function_stats::{FunctionStats, extract_functions};
pub use metrics::{ComplexityLevel, ComplexitySummary, FileComplexity};

use std::path::Path;

use crate::language::Language;

/// 分析单个文件的复杂度
pub fn analyze_file_complexity(
    content: &str,
    path: &Path,
    language: Language,
) -> Option<metrics::FileComplexity> {
    // 预先收集行，避免重复遍历
    let lines: Vec<&str> = content.lines().collect();

    // 提取函数
    let functions = extract_functions(content, language);

    // 如果没有函数，返回 None
    if functions.is_empty() {
        return None;
    }

    // 计算每个函数的圈复杂度（优化：直接使用行范围，不创建新字符串）
    let functions_with_complexity: Vec<FunctionStats> = functions
        .into_iter()
        .map(|func| {
            // 直接使用行范围计算复杂度，避免字符串分配
            let cyclomatic_stats =
                calculate_cyclomatic_complexity_from_lines(&lines, &func, language);
            FunctionStats {
                name: func.name,
                start_line: func.start_line,
                end_line: func.end_line,
                lines: func.lines,
                cyclomatic: cyclomatic_stats.complexity,
                parameter_count: func.parameter_count,
            }
        })
        .collect();

    // 计算文件级别的复杂度统计
    let total_cyclomatic: usize = functions_with_complexity.iter().map(|f| f.cyclomatic).sum();
    let max_cyclomatic = functions_with_complexity
        .iter()
        .map(|f| f.cyclomatic)
        .max()
        .unwrap_or(0);
    let avg_cyclomatic = if functions_with_complexity.is_empty() {
        0.0
    } else {
        total_cyclomatic as f64 / functions_with_complexity.len() as f64
    };

    let total_function_lines: usize = functions_with_complexity.iter().map(|f| f.lines).sum();
    let max_function_length = functions_with_complexity
        .iter()
        .map(|f| f.lines)
        .max()
        .unwrap_or(0);
    let avg_function_length = if functions_with_complexity.is_empty() {
        0.0
    } else {
        total_function_lines as f64 / functions_with_complexity.len() as f64
    };

    Some(metrics::FileComplexity {
        path: path.to_path_buf(),
        cyclomatic: total_cyclomatic,
        avg_cyclomatic,
        max_cyclomatic,
        functions: functions_with_complexity,
        avg_function_length,
        max_function_length,
    })
}

/// 从行数组计算函数的圈复杂度（避免字符串分配）
fn calculate_cyclomatic_complexity_from_lines(
    lines: &[&str],
    func: &FunctionStats,
    language: Language,
) -> CyclomaticStats {
    if func.start_line == 0 || func.end_line == 0 || func.start_line > func.end_line {
        return CyclomaticStats::default();
    }

    let start_idx = func.start_line.saturating_sub(1);
    let end_idx = func.end_line.min(lines.len());

    // 直接遍历行，不创建新字符串
    let mut stats = CyclomaticStats::default();
    let mut current_depth = 0usize;
    let mut max_depth = 0usize;

    for line in &lines[start_idx..end_idx] {
        // 逐行计算复杂度
        let line_stats = calculate_cyclomatic_complexity(line, language);
        stats.decision_points += line_stats.decision_points;

        // 计算嵌套深度（需要跟踪整个函数体）
        for c in line.chars() {
            if c == '{' {
                current_depth += 1;
                if current_depth > max_depth {
                    max_depth = current_depth;
                }
            } else if c == '}' {
                current_depth = current_depth.saturating_sub(1);
            }
        }
    }

    stats.complexity = 1 + stats.decision_points;
    stats.nesting_depth = max_depth;
    stats
}

/// 从汇总的文件复杂度数据计算总体复杂度统计
pub fn calculate_complexity_summary(
    file_complexities: &[metrics::FileComplexity],
) -> ComplexitySummary {
    if file_complexities.is_empty() {
        return ComplexitySummary::default();
    }

    let total_cyclomatic: usize = file_complexities.iter().map(|f| f.cyclomatic).sum();
    let total_functions: usize = file_complexities.iter().map(|f| f.functions.len()).sum();

    let all_functions: Vec<&FunctionStats> = file_complexities
        .iter()
        .flat_map(|f| f.functions.iter())
        .collect();

    let avg_cyclomatic = if total_functions > 0 {
        total_cyclomatic as f64 / total_functions as f64
    } else {
        0.0
    };

    let total_function_lines: usize = all_functions.iter().map(|f| f.lines).sum();
    let avg_function_length = if total_functions > 0 {
        total_function_lines as f64 / total_functions as f64
    } else {
        0.0
    };

    // 高复杂度函数（圈复杂度 > 10）
    let high_complexity_functions = all_functions.iter().filter(|f| f.cyclomatic > 10).count();

    // 长函数（行数 > 50）
    let long_functions = all_functions.iter().filter(|f| f.lines > 50).count();

    ComplexitySummary {
        total_cyclomatic,
        avg_cyclomatic,
        total_functions,
        avg_function_length,
        high_complexity_functions,
        long_functions,
    }
}
