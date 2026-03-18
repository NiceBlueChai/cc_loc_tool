use serde::Serialize;

use crate::language::Language;

/// 函数统计信息
#[derive(Clone, Debug, Serialize)]
pub struct FunctionStats {
    /// 函数名
    pub name: String,
    /// 起始行号（从1开始）
    pub start_line: usize,
    /// 结束行号
    pub end_line: usize,
    /// 函数行数
    pub lines: usize,
    /// 圈复杂度
    pub cyclomatic: usize,
    /// 参数数量
    pub parameter_count: usize,
}

/// 从源代码中提取函数信息
pub fn extract_functions(source: &str, language: Language) -> Vec<FunctionStats> {
    match language {
        Language::C | Language::Cpp => extract_functions_c_style(source),
        Language::Java => extract_functions_java(source),
        Language::Rust => extract_functions_rust(source),
        Language::Go => extract_functions_go(source),
        Language::Python => extract_functions_python(source),
    }
}

/// 提取 C/C++ 风格的函数
fn extract_functions_c_style(source: &str) -> Vec<FunctionStats> {
    let mut functions = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    
    // 用于匹配函数定义的正则模式
    // 匹配: 返回类型 函数名(参数)
    // 例如: int main(), void foo(int x), std::string bar()
    
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        
        // 跳过空行和注释
        if line.is_empty() || line.starts_with("//") || line.starts_with("/*") || line.starts_with("*") {
            i += 1;
            continue;
        }
        
        // 尝试匹配函数定义
        if let Some(func) = try_parse_function_c_style(&lines, i) {
            functions.push(func);
            i += 1;
            continue;
        }
        
        i += 1;
    }
    
    functions
}

/// 尝试解析 C/C++ 风格的函数定义
fn try_parse_function_c_style(lines: &[&str], start_idx: usize) -> Option<FunctionStats> {
    let line = lines[start_idx].trim();
    
    // 检查是否可能是函数定义（包含括号）
    if !line.contains('(') {
        return None;
    }
    
    // 排除常见的非函数情况
    let exclude_keywords = [
        "if(", "if (", "while(", "while (", "for(", "for (",
        "switch(", "switch (", "catch(", "catch (",
        "#define", "#include", "#if", "#ifdef", "#ifndef",
        "class ", "struct ", "enum ", "union ", "namespace ",
    ];
    
    for keyword in &exclude_keywords {
        if line.starts_with(keyword) {
            return None;
        }
    }
    
    // 提取函数名
    let func_name = extract_function_name_c_style(line)?;
    
    // 计算参数数量
    let param_count = count_parameters_c_style(line);
    
    // 查找函数体的开始和结束
    let (start_line, end_line) = find_function_body_range(lines, start_idx)?;
    
    let lines_count = end_line - start_line + 1;
    
    Some(FunctionStats {
        name: func_name,
        start_line: start_line + 1, // 转换为1-based行号
        end_line: end_line + 1,
        lines: lines_count,
        cyclomatic: 1, // 将在后续计算
        parameter_count: param_count,
    })
}

/// 从 C/C++ 函数定义行提取函数名
fn extract_function_name_c_style(line: &str) -> Option<String> {
    // 查找左括号位置
    let paren_pos = line.find('(')?;
    
    // 获取括号前的部分
    let before_paren = &line[..paren_pos];
    
    // 分割成单词，取最后一个作为函数名
    let words: Vec<&str> = before_paren
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .collect();
    
    if words.is_empty() {
        return None;
    }
    
    // 最后一个词可能是函数名（可能包含指针符号）
    let last_word = words.last()?;
    
    // 移除可能的指针符号
    let name = last_word
        .trim_start_matches('*')
        .trim_start_matches('&')
        .to_string();
    
    // 验证函数名有效性
    if name.is_empty() || !name.chars().next()?.is_alphabetic() && !name.starts_with('_') {
        return None;
    }
    
    Some(name)
}

/// 计算 C/C++ 函数的参数数量
fn count_parameters_c_style(line: &str) -> usize {
    // 查找括号内的内容
    let start = match line.find('(') {
        Some(pos) => pos + 1,
        None => return 0,
    };
    
    let end = match line.find(')') {
        Some(pos) => pos,
        None => return 0,
    };
    
    if start >= end {
        return 0;
    }
    
    let params = &line[start..end];
    
    // 空参数列表
    if params.trim().is_empty() {
        return 0;
    }
    
    // 计算逗号数量 + 1
    params.split(',').count()
}

/// 查找函数体的范围（从开始行到结束行）
fn find_function_body_range(lines: &[&str], start_idx: usize) -> Option<(usize, usize)> {
    let mut brace_count = 0;
    let mut found_open_brace = false;
    let mut start_line = start_idx;
    let mut end_line = start_idx;
    
    // 查找函数体的开始（左大括号）
    for i in start_idx..lines.len() {
        let line = lines[i];
        
        for c in line.chars() {
            if c == '{' {
                if !found_open_brace {
                    found_open_brace = true;
                    start_line = i;
                }
                brace_count += 1;
            } else if c == '}' {
                brace_count -= 1;
                if found_open_brace && brace_count == 0 {
                    end_line = i;
                    return Some((start_line, end_line));
                }
            }
        }
    }
    
    // 如果没有找到大括号，可能是一个单行函数或声明
    if !found_open_brace {
        return None;
    }
    
    None
}

/// 提取 Java 方法
fn extract_functions_java(source: &str) -> Vec<FunctionStats> {
    // Java 方法与 C++ 类似，但需要处理类定义
    extract_functions_c_style(source)
}

/// 提取 Rust 函数
fn extract_functions_rust(source: &str) -> Vec<FunctionStats> {
    let mut functions = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        
        // 查找 fn 关键字
        if line.starts_with("fn ") || line.contains(" fn ") {
            if let Some(func) = try_parse_function_rust(&lines, i) {
                functions.push(func);
            }
        }
        
        i += 1;
    }
    
    functions
}

/// 尝试解析 Rust 函数定义
fn try_parse_function_rust(lines: &[&str], start_idx: usize) -> Option<FunctionStats> {
    let line = lines[start_idx].trim();
    
    // 提取函数名
    let fn_pos = line.find("fn ")?;
    let after_fn = &line[fn_pos + 3..];
    
    // 查找函数名（到左括号或 < 之前）
    let name_end = after_fn
        .find(|c| c == '(' || c == '<')
        .unwrap_or(after_fn.len());
    
    let func_name = after_fn[..name_end]
        .trim()
        .trim_end_matches('<')
        .to_string();
    
    if func_name.is_empty() {
        return None;
    }
    
    // 计算参数数量
    let param_count = count_parameters_rust(line);
    
    // 查找函数体范围
    let (start_line, end_line) = find_function_body_range_rust(lines, start_idx)?;
    
    let lines_count = end_line - start_line + 1;
    
    Some(FunctionStats {
        name: func_name,
        start_line: start_line + 1,
        end_line: end_line + 1,
        lines: lines_count,
        cyclomatic: 1,
        parameter_count: param_count,
    })
}

/// 计算 Rust 函数的参数数量
fn count_parameters_rust(line: &str) -> usize {
    let start = match line.find('(') {
        Some(pos) => pos + 1,
        None => return 0,
    };
    
    // Rust 可能有泛型参数，需要找到正确的右括号
    let mut depth = 1;
    let mut end = start;
    let chars: Vec<char> = line.chars().collect();
    
    for i in start..chars.len() {
        match chars[i] {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    
    if start >= end {
        return 0;
    }
    
    let params: String = chars[start..end].iter().collect();
    
    if params.trim().is_empty() || params.trim() == "self" || params.trim() == "&self" || params.trim() == "mut self" {
        return if params.trim().contains("self") { 1 } else { 0 };
    }
    
    // 计算参数数量（简化处理）
    params.split(',').filter(|s| !s.trim().is_empty()).count()
}

/// 查找 Rust 函数体范围
fn find_function_body_range_rust(lines: &[&str], start_idx: usize) -> Option<(usize, usize)> {
    find_function_body_range(lines, start_idx)
}

/// 提取 Go 函数
fn extract_functions_go(source: &str) -> Vec<FunctionStats> {
    let mut functions = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        
        // 查找 func 关键字
        if line.starts_with("func ") {
            if let Some(func) = try_parse_function_go(&lines, i) {
                functions.push(func);
            }
        }
        
        i += 1;
    }
    
    functions
}

/// 尝试解析 Go 函数定义
fn try_parse_function_go(lines: &[&str], start_idx: usize) -> Option<FunctionStats> {
    let line = lines[start_idx].trim();
    
    // 提取函数名
    let after_func = &line[5..]; // 跳过 "func "
    
    // Go 可能有接收器，如 func (r *Receiver) Method()
    let (func_name, param_count) = if after_func.starts_with('(') {
        // 有接收器的方法
        let receiver_end = after_func.find(')')?;
        let after_receiver = &after_func[receiver_end + 1..].trim_start();
        
        let name_end = after_receiver.find('(').unwrap_or(after_receiver.len());
        let name = after_receiver[..name_end].trim().to_string();
        
        let params = after_receiver.find('(').map(|pos| {
            count_parameters_go(&after_receiver[pos..])
        }).unwrap_or(0);
        
        (name, params)
    } else {
        // 普通函数
        let name_end = after_func.find('(').unwrap_or(after_func.len());
        let name = after_func[..name_end].trim().to_string();
        
        let params = after_func.find('(').map(|pos| {
            count_parameters_go(&after_func[pos..])
        }).unwrap_or(0);
        
        (name, params)
    };
    
    if func_name.is_empty() {
        return None;
    }
    
    // 查找函数体范围
    let (start_line, end_line) = find_function_body_range(lines, start_idx)?;
    
    let lines_count = end_line - start_line + 1;
    
    Some(FunctionStats {
        name: func_name,
        start_line: start_line + 1,
        end_line: end_line + 1,
        lines: lines_count,
        cyclomatic: 1,
        parameter_count: param_count,
    })
}

/// 计算 Go 函数的参数数量
fn count_parameters_go(line: &str) -> usize {
    let start = match line.find('(') {
        Some(pos) => pos + 1,
        None => return 0,
    };
    
    let mut depth = 1;
    let mut end = start;
    let chars: Vec<char> = line.chars().collect();
    
    for i in start..chars.len() {
        match chars[i] {
            '(' | '[' => depth += 1,
            ')' | ']' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    
    if start >= end {
        return 0;
    }
    
    let params: String = chars[start..end].iter().collect();
    
    if params.trim().is_empty() {
        return 0;
    }
    
    params.split(',').filter(|s| !s.trim().is_empty()).count()
}

/// 提取 Python 函数
fn extract_functions_python(source: &str) -> Vec<FunctionStats> {
    let mut functions = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        
        // 查找 def 关键字（注意缩进）
        let trimmed = line.trim();
        if trimmed.starts_with("def ") {
            if let Some(func) = try_parse_function_python(&lines, i) {
                functions.push(func);
            }
        }
        
        i += 1;
    }
    
    functions
}

/// 尝试解析 Python 函数定义
fn try_parse_function_python(lines: &[&str], start_idx: usize) -> Option<FunctionStats> {
    let line = lines[start_idx];
    let trimmed = line.trim();
    
    // 提取函数名
    let after_def = &trimmed[4..]; // 跳过 "def "
    
    let name_end = after_def.find('(').unwrap_or(after_def.len());
    let func_name = after_def[..name_end].trim().to_string();
    
    if func_name.is_empty() {
        return None;
    }
    
    // 计算参数数量
    let param_count = count_parameters_python(trimmed);
    
    // Python 使用缩进，需要找到函数结束位置
    let start_line = start_idx;
    let end_line = find_python_function_end(lines, start_idx);
    
    let lines_count = end_line - start_line + 1;
    
    Some(FunctionStats {
        name: func_name,
        start_line: start_line + 1,
        end_line: end_line + 1,
        lines: lines_count,
        cyclomatic: 1,
        parameter_count: param_count,
    })
}

/// 计算 Python 函数的参数数量
fn count_parameters_python(line: &str) -> usize {
    let start = match line.find('(') {
        Some(pos) => pos + 1,
        None => return 0,
    };
    
    let end = match line.rfind(')') {
        Some(pos) => pos,
        None => return 0,
    };
    
    if start >= end {
        return 0;
    }
    
    let params = &line[start..end];
    
    // 处理 self/cls 参数
    let params = params.trim();
    if params.is_empty() {
        return 0;
    }
    
    // 移除 self 或 cls
    let params = if params.starts_with("self") {
        let after_self = &params[4..].trim_start_matches(',');
        after_self.trim()
    } else if params.starts_with("cls") {
        let after_cls = &params[3..].trim_start_matches(',');
        after_cls.trim()
    } else {
        params
    };
    
    if params.is_empty() {
        return 0;
    }
    
    params.split(',').filter(|s| !s.trim().is_empty()).count()
}

/// 查找 Python 函数的结束行
fn find_python_function_end(lines: &[&str], start_idx: usize) -> usize {
    // 获取函数定义的缩进级别
    let func_indent = lines[start_idx]
        .chars()
        .take_while(|&c| c == ' ' || c == '\t')
        .count();
    
    // 函数体应该比定义多一级缩进
    let body_indent = func_indent + 4; // 假设使用4空格缩进
    
    let mut end_line = start_idx;
    
    for i in (start_idx + 1)..lines.len() {
        let line = lines[i];
        
        // 空行不算
        if line.trim().is_empty() {
            continue;
        }
        
        let current_indent = line
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .count();
        
        // 如果缩进小于等于函数定义的缩进，函数结束
        if current_indent <= func_indent {
            break;
        }
        
        end_line = i;
    }
    
    end_line
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_c_function() {
        let code = r#"
int add(int a, int b) {
    return a + b;
}

int main() {
    return 0;
}
"#;
        let functions = extract_functions_c_style(code);
        assert_eq!(functions.len(), 2);
        assert_eq!(functions[0].name, "add");
        assert_eq!(functions[0].parameter_count, 2);
        assert_eq!(functions[1].name, "main");
    }
    
    #[test]
    fn test_extract_rust_function() {
        let code = r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn main() {
    println!("Hello");
}
"#;
        let functions = extract_functions_rust(code);
        assert_eq!(functions.len(), 2);
        assert_eq!(functions[0].name, "add");
        assert_eq!(functions[1].name, "main");
    }
    
    #[test]
    fn test_extract_python_function() {
        let code = r#"
def add(a, b):
    return a + b

def greet(name):
    print(f"Hello, {name}")
"#;
        let functions = extract_functions_python(code);
        assert_eq!(functions.len(), 2);
        assert_eq!(functions[0].name, "add");
        assert_eq!(functions[1].name, "greet");
    }
    
    #[test]
    fn test_extract_go_function() {
        let code = r#"
func add(a, b int) int {
    return a + b
}

func (r *Receiver) Method() {
    // method body
}
"#;
        let functions = extract_functions_go(code);
        assert_eq!(functions.len(), 2);
        assert_eq!(functions[0].name, "add");
        assert_eq!(functions[1].name, "Method");
    }
}
