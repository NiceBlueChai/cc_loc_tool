# 复杂度详情弹窗添加复制函数签名功能

## 目标
在复杂度详情弹窗中，点击函数名可以复制函数签名到剪贴板

## 需求分析
当前显示内容：
- 函数名
- 行号范围
- 行数
- 圈复杂度
- 参数数量
- 状态（良好/中等/需改进）

用户希望：点击函数名可以复制完整函数签名（如 `void MyClass::method(int param1, int param2)`）

## 实现方案

### 1. 修改 ComplexityDetailView
- 为函数名添加点击事件
- 点击时复制函数签名到剪贴板
- 添加简单的复制反馈（如改变按钮颜色或显示提示）

### 2. 复制内容格式
```
函数名(参数类型1 参数1, 参数类型2 参数2)
```
例如：
- `int main()`
- `void process(int a, int b)`
- `std::string MyClass::getName() const`

### 3. 技术实现
- 使用 `gpui::Clipboard` API（如果可用）
- 或使用系统原生剪贴板

## 实现步骤

1. 修改 `ComplexityDetailView` 的渲染函数
   - 将函数名改为可点击的按钮或带事件的书签
   
2. 添加点击处理逻辑
   - 获取函数签名
   - 复制到剪贴板
   - 显示复制成功提示

## 代码修改

文件：`src/ui/view.rs`
- 修改 `ComplexityDetailView::render` 函数
- 为函数名添加按钮和点击事件
