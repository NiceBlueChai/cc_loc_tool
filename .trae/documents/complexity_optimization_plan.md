# 复杂度分析优化与用户选项计划

## 问题分析

### 性能瓶颈
1. **重复遍历源代码**：函数提取和复杂度计算分别遍历源代码
2. **字符串分配**：`extract_function_content` 为每个函数创建新字符串
3. **无缓存机制**：每次扫描都重新计算所有文件
4. **串行处理**：虽然文件级并行，但单文件内的处理是串行的

### 用户需求
- 添加复选框让用户选择是否启用复杂度分析
- 复杂度分析较慢，用户可能只需要基本统计

## 实现计划

### 第一部分：添加复杂度分析开关

#### 1. 修改状态结构 (src/ui/view.rs)
- 在 `LocToolView` 添加 `analyze_complexity: bool` 字段
- 默认值设为 `false`（性能优先）

#### 2. 添加 UI 复选框 (src/ui/view.rs)
- 在扫描按钮附近添加"复杂度分析"复选框
- 使用 `Checkbox` 组件或 `Button` 切换状态

#### 3. 修改扫描逻辑 (src/ui/view.rs)
- 根据 `analyze_complexity` 选择扫描函数：
  - `true` → `scan_directory_with_complexity`
  - `false` → `scan_directory`

#### 4. 保存用户偏好 (src/config.rs)
- 在 `AppConfig` 添加 `analyze_complexity: bool` 字段
- 自动保存用户选择

### 第二部分：性能优化（可选后续）

#### 优化方案 A：减少字符串分配
- 修改 `extract_function_content` 使用切片引用
- 避免为每个函数创建新字符串

#### 优化方案 B：合并遍历
- 将函数提取和复杂度计算合并为单次遍历
- 在提取函数时同时计算复杂度

#### 优化方案 C：增量分析
- 缓存已分析文件的结果
- 只分析修改过的文件

## 实现步骤

### 步骤 1：修改配置结构
文件：`src/config.rs`
- 添加 `analyze_complexity: bool` 字段
- 默认值 `false`

### 步骤 2：修改 UI 状态
文件：`src/ui/view.rs`
- `LocToolView` 添加 `analyze_complexity` 字段
- 从配置加载初始值

### 步骤 3：添加复选框 UI
文件：`src/ui/view.rs`
- 在 `render_header` 中添加复选框
- 绑定到 `analyze_complexity` 状态

### 步骤 4：修改扫描逻辑
文件：`src/ui/view.rs`
- `scan` 方法根据开关选择扫描函数
- 更新 `LocSummary` 计算方式

### 步骤 5：测试验证
- 编译测试
- 验证开关功能正常
- 验证配置保存

## UI 设计

```
[项目目录] [/path/to/project] [浏览...] [扫描] [深色]
[选择语言] [全选] [全不选] [C] [C++] [Java] [Python] [Go] [Rust]
[排除目录] [build,dist,node_modules]
[排除文件] [moc_*,*.generated.cpp] (支持 * 通配符)
[复杂度分析] ☐ 启用（可能增加扫描时间）
```

## 代码修改清单

| 文件 | 修改内容 |
|------|----------|
| src/config.rs | 添加 analyze_complexity 字段 |
| src/ui/view.rs | 添加状态字段、复选框、条件扫描逻辑 |
| src/ui/state.rs | 无需修改 |
