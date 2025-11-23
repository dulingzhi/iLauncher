# 智能推荐系统

## 概述

智能推荐系统根据用户的使用习惯和上下文，主动提供个性化的搜索建议。

## 推荐策略

### 1. 频率推荐
基于历史使用统计，推荐最常用的项目：
- 按使用次数降序排列
- 分数范围：100 → 95 → 90...（每项递减5分）
- 推荐理由：显示使用次数

### 2. 时间推荐
根据当前时间段，推荐该时段常用的项目：
- **早晨** (6:00-12:00): 工作启动项
- **午间** (12:00-18:00): 日常任务
- **晚间** (18:00-24:00): 娱乐或休闲项目
- 分数范围：70-80（时间匹配度加成）

### 3. 最近推荐
推荐最近访问的项目：
- 按最后访问时间降序
- 分数：60（固定）
- 适合快速回到上次使用的项目

## API 使用

### 获取综合推荐
```typescript
import { invoke } from "@tauri-apps/api/core";

const suggestions = await invoke<Suggestion[]>("get_smart_suggestions", {
  query: "",        // 空字符串表示获取所有推荐
  limit: 10         // 最多返回10条
});
```

### 获取频率推荐
```typescript
const frequent = await invoke<Suggestion[]>("get_frequent_suggestions", {
  limit: 5
});
```

### 获取时间推荐
```typescript
const timeBased = await invoke<Suggestion[]>("get_time_based_suggestions", {
  limit: 5
});
```

### 获取最近推荐
```typescript
const recent = await invoke<Suggestion[]>("get_recent_suggestions", {
  limit: 5
});
```

## 数据结构

### Suggestion
```rust
pub struct Suggestion {
    pub result_id: String,     // 结果ID（唯一标识）
    pub title: String,         // 标题
    pub subtitle: String,      // 副标题
    pub score: u32,            // 推荐分数（0-100）
    pub reason: String,        // 推荐理由
}
```

### SuggestionContext
```rust
pub struct SuggestionContext {
    pub query: Option<String>,           // 当前查询（可选）
    pub current_time: DateTime<Local>,   // 当前时间
    pub recent_queries: Vec<String>,     // 最近的查询历史
}
```

## 前端集成

### SmartSuggestions 组件
```tsx
import { SmartSuggestions } from "@/components/SmartSuggestions";

<SmartSuggestions 
  query={query}
  onSelect={(suggestion) => {
    // 处理选择事件
    executeResult(suggestion.result_id);
  }}
/>
```

### 显示时机
- 搜索框为空时显示
- 瀑布流卡片布局
- 悬停时高亮边框
- 点击执行对应操作

## 推荐优化

### 去重逻辑
同一个 `result_id` 在不同策略中出现时：
- 保留分数最高的推荐
- 合并推荐理由（用分号分隔）

### 分数计算规则
```
综合分数 = 基础分数 + 时间匹配加成 + 频率加成
- 频率推荐: 100, 95, 90, 85, 80...
- 时间推荐: 70-80 + 使用频率
- 最近推荐: 60（固定）
```

## 性能考虑

- **内存缓存**: 统计数据常驻内存
- **异步计算**: 所有推荐算法异步执行
- **限制数量**: 默认最多返回10条推荐
- **快速查询**: 基于索引的排序和过滤

## 未来扩展

1. **语义推荐**: 基于查询语义的相关推荐
2. **协同过滤**: 基于相似用户的推荐
3. **情境感知**: 结合工作模式、网络状态等上下文
4. **学习算法**: ML模型优化推荐准确度
5. **推荐解释**: 更详细的推荐理由展示

## 隐私说明

所有推荐数据均存储在本地：
- 位置：`%LOCALAPPDATA%\iLauncher\statistics.db`
- 不上传云端
- 用户可随时清除统计数据
