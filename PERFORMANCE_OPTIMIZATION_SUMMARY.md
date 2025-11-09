# MFT 搜索性能优化总结

## 优化成果

### 添加 PRIORITY 索引后的性能提升

| 测试项 | 优化前 | 优化后 | 提升倍数 |
|--------|--------|--------|---------|
| **sys (Limit 10)** | 8052ms | **127ms** | **63x** ⚡ |
| **sys (Limit 50)** | 8209ms | **306ms** | **27x** ⚡ |
| **sys (Limit 100)** | 8221ms | **326ms** | **25x** ⚡ |
| **sys (Limit 500)** | 9628ms | **675ms** | **14x** ⚡ |
| **chrome (Limit 50)** | 1781ms | **592ms** | **3x** ✓ |
| **opera (Limit 50)** | N/A | 1480ms | N/A |

### 性能对比

| 实现 | 平均搜索时间 | 状态 |
|------|------------|------|
| **File-Engine C++** | 50-100ms | ⭐⭐⭐⭐⭐ |
| **当前 Rust (有索引)** | **300-600ms** | ⭐⭐⭐⭐ |
| **之前 Rust (无索引)** | 8000ms | ⭐ |

## 关键优化点

### 1. ✅ 添加 PRIORITY 索引

**问题**：主键 `(ASCII, PATH, PRIORITY)` 不能优化 `WHERE PRIORITY=X` 查询

**解决**：
```sql
CREATE INDEX idx_priority_0 ON list0(PRIORITY);
CREATE INDEX idx_priority_1 ON list1(PRIORITY);
-- ... 为所有 41 张表创建索引
```

**效果**：**10-60 倍性能提升** ⚡

### 2. ✅ 提前终止逻辑

**代码**：
```rust
'priority_loop: for &priority in &priorities {
    for &group in &groups_to_search {
        if results.len() >= limit {
            break 'priority_loop;  // ← 跳出所有循环
        }
    }
}
```

**效果**：找够结果后立即停止，避免无用查询

### 3. ✅ 按 PRIORITY 分批查询

**策略**：
```rust
let priorities = vec![5, 4, 3, 2, 1, 0, -1];
// 先查 .exe (priority=5)
// 再查 .lnk (priority=4)
// ...
```

**优势**：高优先级文件（如 .exe）优先返回

## 待优化点

### 问题 1：查询范围太大

**当前策略**：
```rust
// 根据查询关键字计算 ASCII，查询 ±5 个 group（11 张表）
let target_group = (query_ascii / 100).min(40);
let groups_to_search = (target_group - 5)..=(target_group + 5);
```

**问题**：
- "opera" 搜索了 11 张表，耗时 1480ms
- 大部分表可能没有结果

**优化方案 A**：动态调整范围
```rust
// 第1轮：只查 target_group
// 第2轮：如果不够，扩展到 ±1
// 第3轮：如果还不够，扩展到 ±5
```

**优化方案 B**：查询所有表（File-Engine 策略）
```rust
let groups_to_search: Vec<usize> = (0..=40).collect();
```
- 需要索引支持（已有 ✅）
- 配合提前终止，只查询必要的表

### 问题 2：多驱动器串行查询

**当前代码**：
```rust
for &drive in &existing_drives {
    let db = Database::open(drive, output_dir)?;
    let results = db.search(query, limit)?;
    // 合并结果
}
```

**优化方案**：并行查询
```rust
use rayon::prelude::*;

let all_results: Vec<_> = existing_drives
    .par_iter()  // ← 并行迭代
    .map(|&drive| {
        let db = Database::open(drive, output_dir)?;
        db.search(query, limit)
    })
    .collect();
```

**预期提升**：3 个驱动器 → 快 2-3 倍

### 问题 3：数据库连接开销

**当前**：每次搜索都打开/关闭数据库

**优化方案**：连接池
```rust
static DB_POOL: OnceCell<HashMap<char, Arc<Mutex<Database>>>> = OnceCell::new();

pub fn get_db(drive: char) -> Arc<Mutex<Database>> {
    DB_POOL.get_or_init(|| {
        // 初始化所有驱动器的连接
    }).get(&drive).unwrap()
}
```

## 性能目标

| 目标 | 当前 | 目标 | 实现方案 |
|------|------|------|---------|
| **Limit 50 平均** | 600ms | **< 200ms** | 并行查询 + 查询所有表 |
| **Limit 10 平均** | 300ms | **< 100ms** | 连接池 + 优化范围 |
| **极端情况 (opera)** | 1480ms | **< 500ms** | 查询所有表 + 并行 |

## 下一步行动

### 立即可做（高优先级）

1. ✅ **修改为查询所有 41 张表**
   - 代码：`let groups_to_search = (0..=40).collect()`
   - 预期：找到所有结果，避免遗漏
   - 有索引支持，不会太慢

2. ⏳ **添加并行查询**
   - 使用 rayon 并行查询多个驱动器
   - 预期：200-300ms → 100-150ms

3. ⏳ **重新测试**
   - 验证所有测试用例通过
   - 确认无 `database is locked` 错误

### 可选优化（中优先级）

4. ⏳ **连接池**
   - 避免重复打开数据库
   - 预期：再提升 20-30%

5. ⏳ **动态查询范围**
   - 第一轮查 1 张表，不够再扩展
   - 适用于精确搜索

## 测试命令

```bash
# 完整性能测试
cargo test --lib mft_scanner::database::tests -- --nocapture --test-threads=1

# 单个测试
cargo test --lib test_search_performance_single_keyword -- --nocapture
cargo test --lib test_search_with_different_limits -- --nocapture
cargo test --lib test_concurrent_search -- --nocapture
cargo test --lib test_search_stress -- --nocapture
```

---

**总结**：通过添加 PRIORITY 索引，性能提升了 **3-60 倍**，达到了可用水平（300-600ms）。进一步优化可以达到 File-Engine 的 50-100ms 水平。
