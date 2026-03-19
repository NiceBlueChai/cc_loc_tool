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

/// C/C++ 关键字列表（不能作为函数名）
const CPP_KEYWORDS: &[&str] = &[
    "alignas", "alignof", "and", "and_eq", "asm", "auto", "bitand", "bitor",
    "bool", "break", "case", "catch", "char", "char8_t", "char16_t", "char32_t",
    "class", "compl", "concept", "const", "consteval", "constexpr", "const_cast",
    "continue", "co_await", "co_return", "co_yield", "decltype", "default", "delete",
    "do", "double", "dynamic_cast", "else", "enum", "explicit", "export", "extern",
    "false", "float", "for", "friend", "goto", "if", "inline", "int", "long",
    "mutable", "namespace", "new", "noexcept", "not", "not_eq", "nullptr", "operator",
    "or", "or_eq", "private", "protected", "public", "register", "reinterpret_cast",
    "requires", "return", "short", "signed", "sizeof", "static", "static_assert",
    "static_cast", "struct", "switch", "template", "this", "thread_local", "throw",
    "true", "try", "typedef", "typeid", "typename", "union", "unsigned", "using",
    "virtual", "void", "volatile", "wchar_t", "while", "xor", "xor_eq",
    // C 关键字
    "restrict", "_Bool", "_Complex", "_Imaginary",
];

/// C/C++ 类型关键字（可以作为返回类型）
const CPP_TYPE_KEYWORDS: &[&str] = &[
    "void", "int", "char", "short", "long", "float", "double", "bool", "auto",
    "unsigned", "signed", "wchar_t", "char8_t", "char16_t", "char32_t",
];

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
        }
        
        i += 1;
    }
    
    functions
}

/// 检查是否是有效的函数名（支持类成员函数 MyClass::method）
fn is_valid_function_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    
    // 处理类成员函数：MyClass::method 或 ::globalFunc
    // 取最后一个 :: 后面的部分作为实际函数名
    let actual_name = name.rsplit("::").next().unwrap_or(name);
    
    // 检查第一个字符
    let first_char = actual_name.chars().next().unwrap();
    if !first_char.is_alphabetic() && first_char != '_' && first_char != '~' {
        return false;
    }
    
    // 检查其余字符
    for c in actual_name.chars().skip(1) {
        if !c.is_alphanumeric() && c != '_' {
            return false;
        }
    }
    
    // 不能是关键字
    let name_lower = actual_name.to_lowercase();
    if CPP_KEYWORDS.contains(&name_lower.as_str()) {
        return false;
    }
    
    true
}

/// 检查是否可能是返回类型
fn could_be_return_type(word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    
    // 常见的返回类型
    let type_keywords = CPP_TYPE_KEYWORDS;
    if type_keywords.contains(&word.to_lowercase().as_str()) {
        return true;
    }
    
    // 带有指针/引用符号的类型
    let word_clean = word.trim_end_matches('*').trim_end_matches('&');
    if type_keywords.contains(&word_clean.to_lowercase().as_str()) {
        return true;
    }
    
    // 自定义类型通常以大写字母开头或包含 ::
    // 如: std::string, MyClass, MyNamespace::MyClass
    if word.contains("::") {
        return true;
    }
    
    // 模板类型
    if word.contains('<') && word.contains('>') {
        return true;
    }
    
    // 以大写字母开头的自定义类型
    if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
        return true;
    }
    
    // 带有 std:: 前缀的类型
    if word.starts_with("std::") {
        return true;
    }
    
    false
}

/// 检查行是否包含函数调用模式（应该排除）
fn contains_function_call_pattern(line: &str) -> bool {
    // 检查 -> 操作符（指针成员调用）
    if line.contains("->") {
        // 检查 -> 后面是否有函数调用
        if let Some(pos) = line.find("->") {
            let after_arrow = &line[pos + 2..];
            if after_arrow.contains('(') {
                return true;
            }
        }
    }
    
    // 检查 . 操作符（对象成员调用）
    // 需要区分 std::string getName() 这种情况
    let paren_pos = line.find('(');
    if let Some(paren) = paren_pos {
        let before_paren = &line[..paren];
        // 如果括号前有 . 且不是类型限定符（如 std.），则可能是成员调用
        if before_paren.contains('.') {
            // 检查是否是类似 obj.method() 的模式
            let words: Vec<&str> = before_paren.split_whitespace().collect();
            if let Some(last) = words.last() {
                // 如果最后一个词包含 .，可能是成员调用
                if last.contains('.') && !last.ends_with("::") {
                    // 但要排除返回类型中的 . 如 std::string.
                    // 真正的函数定义不会有 obj.method 这种形式
                    let parts: Vec<&str> = last.split('.').collect();
                    if parts.len() >= 2 && !parts[0].is_empty() {
                        // 检查是否是类似 "std::string." 的情况（不太可能）
                        // 或者是 "obj.method" 的情况
                        if !parts[0].contains("::") {
                            return true;
                        }
                    }
                }
            }
        }
    }
    
    // Lambda 表达式
    if line.contains("[]") || line.starts_with('[') {
        return true;
    }
    
    false
}

/// 尝试解析 C/C++ 风格的函数定义
fn try_parse_function_c_style(lines: &[&str], start_idx: usize) -> Option<FunctionStats> {
    let line = lines[start_idx].trim();
    
    // 检查是否包含括号
    if !line.contains('(') {
        return None;
    }
    
    // 排除预处理指令
    if line.starts_with('#') {
        return None;
    }
    
    // 排除控制语句
    let control_keywords = [
        "if(", "if (", "while(", "while (", "for(", "for (",
        "switch(", "switch (", "catch(", "catch (",
    ];
    for keyword in &control_keywords {
        if line.starts_with(keyword) {
            return None;
        }
    }
    
    // 排除类/结构体/枚举定义
    let type_keywords = ["class ", "struct ", "enum ", "union ", "namespace "];
    for keyword in &type_keywords {
        if line.starts_with(keyword) {
            return None;
        }
    }
    
    // 排除函数调用模式
    if contains_function_call_pattern(line) {
        return None;
    }
    
    // 查找左括号位置
    let paren_pos = line.find('(')?;
    
    // 获取括号前的部分
    let before_paren = &line[..paren_pos];
    
    // 分割成单词
    let words: Vec<&str> = before_paren
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .collect();
    
    if words.is_empty() {
        return None;
    }
    
    // 最后一个词是函数名（可能带有指针符号）
    let last_word = words.last()?;
    let func_name = last_word
        .trim_start_matches('*')
        .trim_start_matches('&')
        .to_string();
    
    // 验证函数名有效性
    if !is_valid_function_name(&func_name) {
        return None;
    }
    
    // 检查是否有返回类型（至少需要一个词在函数名之前）
    // 特殊情况：构造函数、析构函数、类型转换运算符可能没有显式返回类型
    let has_return_type = if words.len() >= 2 {
        // 检查倒数第二个词是否可能是返回类型
        let second_last = words[words.len() - 2];
        could_be_return_type(second_last) || second_last.ends_with("::")
    } else {
        // 单词情况：可能是构造函数或析构函数
        // 如: MyClass() 或 ~MyClass()
        func_name.starts_with('~') || func_name.chars().next()?.is_uppercase()
    };
    
    // 如果没有返回类型且不是构造/析构函数，跳过
    if !has_return_type && words.len() < 2 {
        return None;
    }
    
    // 计算参数数量
    let param_count = count_parameters_c_style(line);
    
    // 查找函数体的开始和结束
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

/// 计算 C/C++ 函数的参数数量
fn count_parameters_c_style(line: &str) -> usize {
    let start = match line.find('(') {
        Some(pos) => pos + 1,
        None => return 0,
    };
    
    let mut depth = 1;
    let mut end = start;
    let chars: Vec<char> = line.chars().collect();
    
    for i in start..chars.len() {
        match chars[i] {
            '(' | '<' | '[' | '{' => depth += 1,
            ')' | '>' | ']' | '}' => {
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
    
    // 计算参数数量（简化处理，不考虑模板参数中的逗号）
    params.split(',').filter(|s| !s.trim().is_empty()).count()
}

/// 查找函数体的范围（从开始行到结束行）
fn find_function_body_range(lines: &[&str], start_idx: usize) -> Option<(usize, usize)> {
    let mut brace_count = 0;
    let mut found_open_brace = false;
    let mut start_line = start_idx;
    let mut end_line = start_idx;
    
    for i in start_idx..lines.len() {
        let line = lines[i];
        let mut in_string = false;
        let mut in_char = false;
        let mut in_line_comment = false;
        
        for (j, c) in line.chars().enumerate() {
            // 处理注释
            if !in_string && !in_char {
                if !in_line_comment && j > 0 {
                    let prev_char = line.chars().nth(j - 1);
                    if c == '/' && prev_char == Some('/') {
                        in_line_comment = true;
                        continue;
                    }
                }
                if in_line_comment {
                    continue;
                }
            }
            
            // 处理字符串
            if c == '"' && !in_char {
                // 检查是否被转义
                let escaped = j > 0 && line.chars().nth(j - 1) == Some('\\');
                if !escaped {
                    in_string = !in_string;
                }
                continue;
            }
            
            // 处理字符
            if c == '\'' && !in_string {
                let escaped = j > 0 && line.chars().nth(j - 1) == Some('\\');
                if !escaped {
                    in_char = !in_char;
                }
                continue;
            }
            
            if in_string || in_char {
                continue;
            }
            
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
    
    if !found_open_brace {
        return None;
    }
    
    None
}

/// 提取 Java 方法
fn extract_functions_java(source: &str) -> Vec<FunctionStats> {
    extract_functions_c_style(source)
}

/// 提取 Rust 函数
fn extract_functions_rust(source: &str) -> Vec<FunctionStats> {
    let mut functions = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        
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
    
    let fn_pos = line.find("fn ")?;
    let after_fn = &line[fn_pos + 3..];
    
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
    
    let param_count = count_parameters_rust(line);
    
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

/// 计算 Rust 函数的参数数量
fn count_parameters_rust(line: &str) -> usize {
    let start = match line.find('(') {
        Some(pos) => pos + 1,
        None => return 0,
    };
    
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
    
    params.split(',').filter(|s| !s.trim().is_empty()).count()
}

/// 提取 Go 函数
fn extract_functions_go(source: &str) -> Vec<FunctionStats> {
    let mut functions = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        
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
    
    let after_func = &line[5..];
    
    let (func_name, param_count) = if after_func.starts_with('(') {
        let receiver_end = after_func.find(')')?;
        let after_receiver = &after_func[receiver_end + 1..].trim_start();
        
        let name_end = after_receiver.find('(').unwrap_or(after_receiver.len());
        let name = after_receiver[..name_end].trim().to_string();
        
        let params = after_receiver.find('(').map(|pos| {
            count_parameters_go(&after_receiver[pos..])
        }).unwrap_or(0);
        
        (name, params)
    } else {
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
    
    let after_def = &trimmed[4..];
    
    let name_end = after_def.find('(').unwrap_or(after_def.len());
    let func_name = after_def[..name_end].trim().to_string();
    
    if func_name.is_empty() {
        return None;
    }
    
    let param_count = count_parameters_python(trimmed);
    
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
    
    let params = params.trim();
    if params.is_empty() {
        return 0;
    }
    
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
    let func_indent = lines[start_idx]
        .chars()
        .take_while(|&c| c == ' ' || c == '\t')
        .count();
    
    let mut end_line = start_idx;
    
    for i in (start_idx + 1)..lines.len() {
        let line = lines[i];
        
        if line.trim().is_empty() {
            continue;
        }
        
        let current_indent = line
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .count();
        
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
    fn test_exclude_function_calls() {
        let code = r#"
void process() {
    obj->method();
    obj.method();
    func();
}
"#;
        let functions = extract_functions_c_style(code);
        // 只应该识别 process 函数
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "process");
    }
    
    #[test]
    fn test_exclude_control_statements() {
        let code = r#"
void test() {
    if (condition) {}
    while (x > 0) {}
    for (int i = 0; i < 10; i++) {}
}
"#;
        let functions = extract_functions_c_style(code);
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "test");
    }
    
    #[test]
    fn test_constructor_destructor() {
        let code = r#"
MyClass::MyClass() {
}

MyClass::~MyClass() {
}
"#;
        let functions = extract_functions_c_style(code);
        assert_eq!(functions.len(), 2);
        // 函数名包含类限定符
        assert_eq!(functions[0].name, "MyClass::MyClass");
        assert_eq!(functions[1].name, "MyClass::~MyClass");
    }
    
    #[test]
    fn test_class_method() {
        let code = r#"
std::string getName() const {
    return name;
}

void MyClass::setValue(int v) {
    value = v;
}
"#;
        let functions = extract_functions_c_style(code);
        assert_eq!(functions.len(), 2);
        assert_eq!(functions[0].name, "getName");
        // 函数名包含类限定符
        assert_eq!(functions[1].name, "MyClass::setValue");
    }
    
    #[test]
    fn test_exclude_lambda() {
        let code = r#"
void test() {
    auto lambda = [](){};
    auto func = [](int x){ return x * 2; };
}
"#;
        let functions = extract_functions_c_style(code);
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "test");
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
    
    #[test]
    fn test_is_valid_function_name() {
        assert!(is_valid_function_name("main"));
        assert!(is_valid_function_name("MyFunction"));
        assert!(is_valid_function_name("_private"));
        assert!(is_valid_function_name("~MyClass"));
        assert!(!is_valid_function_name(""));  // 空
        assert!(!is_valid_function_name("123abc"));  // 数字开头
        assert!(!is_valid_function_name("if"));  // 关键字
        assert!(!is_valid_function_name("while"));  // 关键字
    }
    
    #[test]
    fn test_could_be_return_type() {
        assert!(could_be_return_type("int"));
        assert!(could_be_return_type("void"));
        assert!(could_be_return_type("std::string"));
        assert!(could_be_return_type("MyClass"));
        assert!(could_be_return_type("std::vector<int>"));
        assert!(!could_be_return_type("if"));
        assert!(!could_be_return_type("while"));
    }
}
