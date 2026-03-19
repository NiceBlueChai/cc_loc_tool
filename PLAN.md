# 修复 C/C++ 函数解析问题

## 问题描述
当前的函数解析逻辑存在问题，会错误地将以下情况识别为函数定义：
1. 函数调用：`a->b()`, `obj.method()`, `func()`
2. Lambda 表达式：`[](){}`
3. 宏调用：`SOME_MACRO(args)`
4. 初始化列表：`int arr[] = {1, 2, 3}`

## 根本原因
`extract_function_name_c_style` 函数只是简单地：
1. 检查行是否包含 `(`
2. 取括号前的最后一个词作为函数名
3. 没有验证是否是真正的函数定义

## 解决方案

### 1. 增强函数定义识别规则
真正的函数定义应该满足：
- 有返回类型（或构造/析构函数）
- 函数名是有效的标识符
- 有参数列表（可以为空）
- 有函数体 `{}` 或是纯声明 `;`

### 2. 排除规则
需要排除以下模式：
- 包含 `->` 的行（方法调用）
- 包含 `.` 后跟标识符和 `(` 的（成员函数调用）
- 以 `}` 结尾的单行语句
- Lambda 表达式 `[]`
- 宏定义和宏调用
- 控制语句（if/while/for/switch）

### 3. 改进的解析逻辑
```
1. 检查行是否可能是函数定义
   - 必须包含 `(`
   - 不能以排除关键字开头
   - 不能包含 `->` 或 `.` 在函数名位置
   
2. 验证函数定义格式
   - 格式: [修饰符] 返回类型 函数名(参数) [const/override] { 或 ;
   - 构造函数/析构函数: 类名(参数) 或 ~类名(参数)
   
3. 提取函数名
   - 必须是有效的标识符
   - 不能是关键字
   
4. 查找函数体
   - 必须找到 `{` 和匹配的 `}`
```

## 实现步骤

1. 重写 `try_parse_function_c_style` 函数
   - 添加更严格的验证逻辑
   - 使用正则表达式或更精确的模式匹配
   
2. 添加辅助函数
   - `is_valid_function_name()` - 验证函数名有效性
   - `has_return_type()` - 检查是否有返回类型
   - `is_constructor_or_destructor()` - 检查是否是构造/析构函数
   
3. 增加排除规则
   - 排除方法调用模式
   - 排除 Lambda 表达式
   - 排除控制语句

4. 添加单元测试
   - 测试正常函数定义
   - 测试应排除的情况
   - 测试边界情况

## 测试用例

### 应该识别为函数
```cpp
int main() {}
void foo(int x) {}
std::string getName() const {}
MyClass::MyClass() {}
MyClass::~MyClass() {}
static int count() {}
template<typename T> T max(T a, T b) {}
```

### 不应该识别为函数
```cpp
obj->method()
obj.method()
func()
int arr[] = {1, 2, 3}
auto lambda = [](){};
if (condition) {}
while (x > 0) {}
SOME_MACRO(args)
```
