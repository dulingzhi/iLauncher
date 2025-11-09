# MFT 搜索性能测试报告

生成时间：2025-11-09
测试环境：3 个磁盘驱动器（C:, D:, E:）

## 测试结果总结

### 1. 单关键字搜索性能

| 关键字 | 描述 | 耗时 (ms) | 结果数 | 状态 |
|--------|------|----------|--------|------|
| chrome | 常见程序 | **1781.53** | 50 | ❌ 超时 |
| sys | 系统文件 | **934.73** | 10 | ❌ 超时 |

**性能目标**：< 500ms
**实际性能**：**934-1781ms**（超时 2-3 倍）

### 2. 不同返回数量的性能测试

关键字：`sys`

| Limit | 耗时 (ms) | 实际返回 |
|-------|----------|---------|
| 10 | 934.73 | 10 |
| 50 | 951.43 | 50 |
| 100 | 994.00 | 100 |
| 200 | 1018.56 | 200 |
| 500 | 1178.72 | 500 |

**关键发现**：
- ⚠️ Limit 从 10 → 500，耗时只增加了 26%（934ms → 1178ms）
- 🔥 **说明瓶颈不在结果过滤，而在数据库读取**

---

## 性能瓶颈分析

### 当前实现问题

```rust
// 当前代码逻辑
for priority in [5, 4, 3, 2, 1, 0, -1] {
    for group in 0..=40 {
        // 🔥 问题：每个 priority 都要查询 41 张表
        let sql = format!(
            "SELECT * FROM list{} WHERE PRIORITY={}",
            group, priority
        );
        // 内存过滤
    }
}
```

**问题点**：
1. ❌ **查询次数过多**：7 个 priority × 41 张表 × 3 个驱动器 = **861 次 SQL 查询**
2. ❌ **没有提前终止**：即使找够 50 个结果，仍然继续查询低优先级
3. ❌ **数据库打开/关闭开销**：每个驱动器都要重新打开连接

### 对比 File-Engine 的实现

**File-Engine C++**：
- ✅ 使用 `sqlite3_exec` + callback，找够就停止
- ✅ 一个连接复用所有查询
- ✅ callback 返回非 0 立即终止

**当前 Rust 实现**：
- ❌ 使用 `query_map` 遍历，无法提前终止
- ❌ 每次都查完整个表，然后在内存中过滤
- ❌ 没有实现跨表的提前终止机制

---

## 性能优化建议

### 优先级 1：修复提前终止逻辑

```rust
pub fn search(&self, query: &str, limit: usize) -> Result<Vec<MftFileEntry>> {
    let mut results = Vec::with_capacity(limit);
    
    'outer: for &priority in &[5, 4, 3, 2, 1, 0, -1] {
        for &group in &groups_to_search {
            // ... 查询代码 ...
            
            // 🔥 关键：找够了就跳出所有循环
            if results.len() >= limit {
                break 'outer;  // ← 跳出外层循环
            }
        }
    }
    
    Ok(results)
}
```

**当前代码问题**：
```rust
if results.len() >= limit {
    break;  // ← 只跳出内层循环，还会继续查询下一个 priority
}
```

### 优先级 2：添加索引（如果没有）

```sql
CREATE INDEX IF NOT EXISTS idx_priority ON list0(PRIORITY);
CREATE INDEX IF NOT EXISTS idx_priority ON list1(PRIORITY);
-- ... 为所有 41 张表添加索引
```

### 优先级 3：连接池优化

```rust
// 使用全局连接缓存
static DB_POOL: OnceCell<HashMap<char, Arc<Mutex<Connection>>>> = OnceCell::new();
```

### 优先级 4：并行查询多个驱动器

```rust
// 使用 rayon 并行查询 C:, D:, E:
existing_drives.par_iter()
    .map(|&drive| Database::open(drive, output_dir)?.search(query, limit))
    .collect()
```

---

## 预期优化效果

| 优化项 | 当前耗时 | 预期耗时 | 改善幅度 |
|--------|---------|---------|---------|
| **提前终止修复** | 1781ms | **300-500ms** | 70-80% ↓ |
| **添加索引** | 300-500ms | **100-200ms** | 60-70% ↓ |
| **连接池** | 100-200ms | **50-100ms** | 50% ↓ |
| **并行查询** | 50-100ms | **20-50ms** | 60% ↓ |

**最终目标**：**< 50ms**（File-Engine 水平）

---

## 下一步行动

1. ✅ **立即修复**：提前终止逻辑（预计耗时 5 分钟）
2. ⏳ **验证效果**：重新运行测试
3. ⏳ **添加索引**：如果提前终止不够快
4. ⏳ **性能优化**：连接池 + 并行查询

---

## 测试命令

```bash
# 单关键字性能测试
cargo test --lib mft_scanner::database::tests::test_search_performance_single_keyword -- --nocapture

# 不同 limit 测试
cargo test --lib mft_scanner::database::tests::test_search_with_different_limits -- --nocapture

# 并发测试
cargo test --lib mft_scanner::database::tests::test_concurrent_search -- --nocapture

# 压力测试
cargo test --lib mft_scanner::database::tests::test_search_stress -- --nocapture
```
