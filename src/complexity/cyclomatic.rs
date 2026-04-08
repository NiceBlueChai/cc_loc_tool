#![allow(clippy::collapsible_if)]

use serde::Serialize;

use crate::language::Language;

/// 圈复杂度统计
#[derive(Clone, Debug, Serialize)]
pub struct CyclomaticStats {
    /// 圈复杂度值
    pub complexity: usize,
    /// 决策点数量
    pub decision_points: usize,
    /// 最大嵌套深度
    pub nesting_depth: usize,
}

impl Default for CyclomaticStats {
    fn default() -> Self {
        Self {
            complexity: 1, // 基础复杂度为 1
            decision_points: 0,
            nesting_depth: 0,
        }
    }
}

/// 计算代码的圈复杂度
///
/// 圈复杂度 = 1 + 决策点数量
///
/// 决策点包括：
/// - if/else if 语句
/// - for/while/do-while 循环
/// - switch/case 语句（每个 case）
/// - catch 语句
/// - && 和 || 运算符
/// - ? : 三元运算符
pub fn calculate_cyclomatic_complexity(source: &str, language: Language) -> CyclomaticStats {
    let mut stats = CyclomaticStats::default();
    let mut current_depth = 0usize;
    let mut max_depth = 0usize;

    // 用于跟踪代码块状态
    let mut in_string = false;
    let mut in_char = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut string_escape = false;

    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();

    for i in 0..len {
        let c = chars[i];
        let next_c = if i + 1 < len {
            Some(chars[i + 1])
        } else {
            None
        };
        let prev_c = if i > 0 { Some(chars[i - 1]) } else { None };

        // 处理字符串和注释
        if !in_line_comment && !in_block_comment {
            // 字符串处理
            if c == '"' && !in_char && prev_c != Some('\\') && !string_escape {
                in_string = !in_string;
            }
            // 字符字面量处理
            if c == '\'' && !in_string && prev_c != Some('\\') && !string_escape {
                in_char = !in_char;
            }
            // 转义字符处理
            if c == '\\' && (in_string || in_char) {
                string_escape = !string_escape;
            } else {
                string_escape = false;
            }
        }

        // 注释处理
        if !in_string && !in_char {
            // 行注释
            if c == '/' && next_c == Some('/') && !in_block_comment {
                in_line_comment = true;
                continue;
            }
            // 块注释开始
            if c == '/' && next_c == Some('*') && !in_line_comment {
                in_block_comment = true;
                continue;
            }
            // 块注释结束
            if c == '*' && next_c == Some('/') && in_block_comment {
                in_block_comment = false;
                continue;
            }
        }

        // 跳过字符串、字符字面量和注释中的内容
        if in_string || in_char || in_line_comment || in_block_comment {
            // 行注释在换行时结束
            if c == '\n' {
                in_line_comment = false;
            }
            continue;
        }

        // 根据语言计算决策点
        match language {
            Language::C | Language::Cpp | Language::Java | Language::Rust | Language::Go => {
                // 检测关键字和运算符
                stats.decision_points += count_decision_points_c_style(&chars, i, language);

                // 计算嵌套深度
                if c == '{' {
                    current_depth += 1;
                    if current_depth > max_depth {
                        max_depth = current_depth;
                    }
                } else if c == '}' {
                    current_depth = current_depth.saturating_sub(1);
                }
            }
            Language::Python => {
                // Python 使用缩进，需要特殊处理
                stats.decision_points += count_decision_points_python(&chars, i);
            }
        }
    }

    stats.complexity = 1 + stats.decision_points;
    stats.nesting_depth = max_depth;
    stats
}

/// 计算 C 风格语言的决策点
fn count_decision_points_c_style(chars: &[char], pos: usize, language: Language) -> usize {
    let len = chars.len();
    let c = chars[pos];

    // 检查是否是标识符的开始
    let is_ident_char = |ch: char| ch.is_alphanumeric() || ch == '_';

    // 检查前面的字符是否是标识符字符或空白
    let prev_is_ident_or_space = |offset: usize| {
        if offset == 0 {
            true
        } else {
            let prev = chars[offset - 1];
            !is_ident_char(prev)
        }
    };

    // 检查后面的字符是否是标识符字符或空白
    let next_is_ident_or_space = |offset: usize| {
        if offset >= len {
            true
        } else {
            let next = chars[offset];
            !is_ident_char(next)
        }
    };

    // 检测 if 关键字
    if c == 'i' && pos + 1 < len && chars[pos + 1] == 'f' {
        if prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 2) {
            return 1;
        }
    }

    // 检测 else if 关键字
    if c == 'e' && pos + 6 < len {
        let slice: String = chars[pos..pos + 7].iter().collect();
        if slice == "else if" && prev_is_ident_or_space(pos) {
            return 1;
        }
    }

    // 检测 for 关键字
    if c == 'f' && pos + 2 < len && chars[pos + 1] == 'o' && chars[pos + 2] == 'r' {
        if prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 3) {
            return 1;
        }
    }

    // 检测 while 关键字
    if c == 'w' && pos + 4 < len {
        let slice: String = chars[pos..pos + 5].iter().collect();
        if slice == "while" && prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 5) {
            return 1;
        }
    }

    // 检测 do 关键字（do-while 循环）
    if c == 'd' && pos + 1 < len && chars[pos + 1] == 'o' {
        if prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 2) {
            return 1;
        }
    }

    // 检测 switch 关键字
    if c == 's' && pos + 5 < len {
        let slice: String = chars[pos..pos + 6].iter().collect();
        if slice == "switch" && prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 6) {
            return 1;
        }
    }

    // 检测 case 关键字（switch 的每个 case）
    if c == 'c' && pos + 3 < len {
        let slice: String = chars[pos..pos + 4].iter().collect();
        if slice == "case" && prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 4) {
            return 1;
        }
    }

    // 检测 catch 关键字
    if c == 'c' && pos + 4 < len {
        let slice: String = chars[pos..pos + 5].iter().collect();
        if slice == "catch" && prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 5) {
            return 1;
        }
    }

    // 检测 match 关键字（Rust 特有）
    if language == Language::Rust && c == 'm' && pos + 4 < len {
        let slice: String = chars[pos..pos + 5].iter().collect();
        if slice == "match" && prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 5) {
            return 1;
        }
    }

    // 检测 && 和 || 运算符
    if (c == '&' && pos + 1 < len && chars[pos + 1] == '&')
        || (c == '|' && pos + 1 < len && chars[pos + 1] == '|')
    {
        return 1;
    }

    // 检测三元运算符 ?
    if c == '?' {
        // 排除 ? 作为其他用途的情况（如 Rust 的 ? 操作符）
        if language != Language::Rust {
            return 1;
        }
    }

    0
}

/// 计算 Python 的决策点
fn count_decision_points_python(chars: &[char], pos: usize) -> usize {
    let len = chars.len();
    let c = chars[pos];

    let is_ident_char = |ch: char| ch.is_alphanumeric() || ch == '_';

    let prev_is_ident_or_space = |offset: usize| {
        if offset == 0 {
            true
        } else {
            let prev = chars[offset - 1];
            !is_ident_char(prev)
        }
    };

    let next_is_ident_or_space = |offset: usize| {
        if offset >= len {
            true
        } else {
            let next = chars[offset];
            !is_ident_char(next)
        }
    };

    // 检测 if 关键字
    if c == 'i' && pos + 1 < len && chars[pos + 1] == 'f' {
        if prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 2) {
            return 1;
        }
    }

    // 检测 elif 关键字
    if c == 'e' && pos + 3 < len {
        let slice: String = chars[pos..pos + 4].iter().collect();
        if slice == "elif" && prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 4) {
            return 1;
        }
    }

    // 检测 for 关键字
    if c == 'f' && pos + 2 < len && chars[pos + 1] == 'o' && chars[pos + 2] == 'r' {
        if prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 3) {
            return 1;
        }
    }

    // 检测 while 关键字
    if c == 'w' && pos + 4 < len {
        let slice: String = chars[pos..pos + 5].iter().collect();
        if slice == "while" && prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 5) {
            return 1;
        }
    }

    // 检测 except 关键字（Python 的异常处理）
    if c == 'e' && pos + 5 < len {
        let slice: String = chars[pos..pos + 6].iter().collect();
        if slice == "except" && prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 6) {
            return 1;
        }
    }

    // 检测 and 和 or 关键字（Python 的逻辑运算符）
    if c == 'a' && pos + 2 < len {
        let slice: String = chars[pos..pos + 3].iter().collect();
        if slice == "and" && prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 3) {
            return 1;
        }
    }

    if c == 'o' && pos + 1 < len && chars[pos + 1] == 'r' {
        if prev_is_ident_or_space(pos) && next_is_ident_or_space(pos + 2) {
            return 1;
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_function() {
        let code = r#"
            int add(int a, int b) {
                return a + b;
            }
        "#;
        let stats = calculate_cyclomatic_complexity(code, Language::Cpp);
        assert_eq!(stats.complexity, 1);
        assert_eq!(stats.decision_points, 0);
    }

    #[test]
    fn test_if_statement() {
        let code = r#"
            int max(int a, int b) {
                if (a > b) {
                    return a;
                } else {
                    return b;
                }
            }
        "#;
        let stats = calculate_cyclomatic_complexity(code, Language::Cpp);
        assert_eq!(stats.complexity, 2);
        assert_eq!(stats.decision_points, 1);
    }

    #[test]
    fn test_nested_if() {
        let code = r#"
            int classify(int x) {
                if (x > 0) {
                    if (x > 10) {
                        return 2;
                    } else {
                        return 1;
                    }
                } else {
                    return 0;
                }
            }
        "#;
        let stats = calculate_cyclomatic_complexity(code, Language::Cpp);
        assert_eq!(stats.complexity, 3);
        assert_eq!(stats.nesting_depth, 3);
    }

    #[test]
    fn test_for_loop() {
        let code = r#"
            int sum(int n) {
                int result = 0;
                for (int i = 1; i <= n; i++) {
                    result += i;
                }
                return result;
            }
        "#;
        let stats = calculate_cyclomatic_complexity(code, Language::Cpp);
        assert_eq!(stats.complexity, 2);
    }

    #[test]
    fn test_logical_operators() {
        let code = r#"
            bool check(int a, int b, int c) {
                if (a > 0 && b > 0 || c > 0) {
                    return true;
                }
                return false;
            }
        "#;
        let stats = calculate_cyclomatic_complexity(code, Language::Cpp);
        assert_eq!(stats.complexity, 4); // 1 + if + && + ||
    }
}
