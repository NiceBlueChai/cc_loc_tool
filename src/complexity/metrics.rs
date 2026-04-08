use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::function_stats::FunctionStats;

/// 复杂度等级
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum ComplexityLevel {
    /// 良好（圈复杂度 1-10）
    Good,
    /// 一般（圈复杂度 11-20）
    Moderate,
    /// 较差（圈复杂度 > 20）
    Poor,
}

impl ComplexityLevel {
    /// 根据圈复杂度值判断等级
    pub fn from_complexity(complexity: usize) -> Self {
        match complexity {
            1..=10 => Self::Good,
            11..=20 => Self::Moderate,
            _ => Self::Poor,
        }
    }

    /// 获取等级名称
    pub fn name(&self) -> &'static str {
        match self {
            Self::Good => "良好",
            Self::Moderate => "一般",
            Self::Poor => "较差",
        }
    }
}

/// 函数长度等级
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum FunctionLengthLevel {
    /// 短函数（1-20行）
    Short,
    /// 中等函数（21-50行）
    Medium,
    /// 长函数（>50行）
    Long,
}

#[allow(dead_code)]
impl FunctionLengthLevel {
    /// 根据行数判断等级
    pub fn from_lines(lines: usize) -> Self {
        match lines {
            1..=20 => Self::Short,
            21..=50 => Self::Medium,
            _ => Self::Long,
        }
    }

    /// 获取等级名称
    pub fn name(&self) -> &'static str {
        match self {
            Self::Short => "短",
            Self::Medium => "中等",
            Self::Long => "长",
        }
    }
}

/// 文件复杂度统计
#[derive(Clone, Debug, Serialize)]
pub struct FileComplexity {
    /// 文件路径
    pub path: PathBuf,
    /// 文件总圈复杂度
    pub cyclomatic: usize,
    /// 平均圈复杂度
    pub avg_cyclomatic: f64,
    /// 最大圈复杂度
    pub max_cyclomatic: usize,
    /// 函数列表
    pub functions: Vec<FunctionStats>,
    /// 平均函数长度
    pub avg_function_length: f64,
    /// 最大函数长度
    pub max_function_length: usize,
}

impl FileComplexity {
    /// 获取文件复杂度等级
    pub fn complexity_level(&self) -> ComplexityLevel {
        ComplexityLevel::from_complexity(self.avg_cyclomatic as usize)
    }

    /// 获取高复杂度函数数量
    pub fn high_complexity_count(&self) -> usize {
        self.functions.iter().filter(|f| f.cyclomatic > 10).count()
    }

    /// 获取长函数数量
    pub fn long_function_count(&self) -> usize {
        self.functions.iter().filter(|f| f.lines > 50).count()
    }
}

/// 复杂度汇总统计
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComplexitySummary {
    /// 总圈复杂度
    pub total_cyclomatic: usize,
    /// 平均圈复杂度
    pub avg_cyclomatic: f64,
    /// 总函数数
    pub total_functions: usize,
    /// 平均函数长度
    pub avg_function_length: f64,
    /// 高复杂度函数数量（圈复杂度 > 10）
    pub high_complexity_functions: usize,
    /// 长函数数量（行数 > 50）
    pub long_functions: usize,
}

impl ComplexitySummary {
    /// 获取整体复杂度等级
    pub fn complexity_level(&self) -> ComplexityLevel {
        ComplexityLevel::from_complexity(self.avg_cyclomatic as usize)
    }

    /// 获取高复杂度函数比例
    pub fn high_complexity_ratio(&self) -> f64 {
        if self.total_functions == 0 {
            return 0.0;
        }
        self.high_complexity_functions as f64 / self.total_functions as f64
    }

    /// 获取长函数比例
    pub fn long_function_ratio(&self) -> f64 {
        if self.total_functions == 0 {
            return 0.0;
        }
        self.long_functions as f64 / self.total_functions as f64
    }

    /// 从文件复杂度列表计算汇总
    pub fn from_files(files: &[FileComplexity]) -> Self {
        if files.is_empty() {
            return Self::default();
        }

        let total_cyclomatic: usize = files.iter().map(|f| f.cyclomatic).sum();
        let total_functions: usize = files.iter().map(|f| f.functions.len()).sum();

        let all_functions: Vec<&FunctionStats> =
            files.iter().flat_map(|f| f.functions.iter()).collect();

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

        let high_complexity_functions = all_functions.iter().filter(|f| f.cyclomatic > 10).count();

        let long_functions = all_functions.iter().filter(|f| f.lines > 50).count();

        Self {
            total_cyclomatic,
            avg_cyclomatic,
            total_functions,
            avg_function_length,
            high_complexity_functions,
            long_functions,
        }
    }
}

/// 代码质量报告
#[allow(dead_code)]
#[derive(Clone, Debug, Serialize)]
pub struct QualityReport {
    /// 复杂度汇总
    pub summary: ComplexitySummary,
    /// 需要关注的函数（高复杂度或长函数）
    pub attention_functions: Vec<AttentionFunction>,
}

/// 需要关注的函数
#[allow(dead_code)]
#[derive(Clone, Debug, Serialize)]
pub struct AttentionFunction {
    /// 文件路径
    pub file_path: PathBuf,
    /// 函数名
    pub function_name: String,
    /// 起始行
    pub start_line: usize,
    /// 圈复杂度
    pub cyclomatic: usize,
    /// 函数行数
    pub lines: usize,
    /// 问题类型
    pub issues: Vec<String>,
}

#[allow(dead_code)]
impl QualityReport {
    /// 从文件复杂度列表生成质量报告
    pub fn from_files(files: &[FileComplexity]) -> Self {
        let summary = ComplexitySummary::from_files(files);

        let attention_functions: Vec<AttentionFunction> = files
            .iter()
            .flat_map(|file| {
                file.functions.iter().filter_map(|func| {
                    let mut issues = Vec::new();

                    if func.cyclomatic > 10 {
                        issues.push(format!("高圈复杂度: {}", func.cyclomatic));
                    }
                    if func.lines > 50 {
                        issues.push(format!("函数过长: {} 行", func.lines));
                    }

                    if issues.is_empty() {
                        None
                    } else {
                        Some(AttentionFunction {
                            file_path: file.path.clone(),
                            function_name: func.name.clone(),
                            start_line: func.start_line,
                            cyclomatic: func.cyclomatic,
                            lines: func.lines,
                            issues,
                        })
                    }
                })
            })
            .collect();

        Self {
            summary,
            attention_functions,
        }
    }
}
