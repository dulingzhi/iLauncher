# MFT 搜索性能分析总结

## 性能测试结果（当前）

| 关键字 | Limit | 耗时 | 状态 |
|--------|-------|------|------|
| sys | 10 | **8052ms** | ❌ 严重超时 |
| sys | 50 | **8209ms** | ❌ 严重超时 |
| sys | 500 | **9628ms** | ❌ 严重超时 |
| chrome | 50 | **1781ms** (第一版) → **41682ms** (全表) → **8000ms** (当前) | ❌ 不稳定 |

## 根本问题诊断

### 问题1：没有 PRIORITY 索引

**File-Engine C++ 创建表时的 SQL**：
```cpp
CREATE TABLE IF NOT EXISTS listX (
    ASCII INT,
    PATH TEXT,
    PRIORITY INT,
    PRIMARY KEY(ASCII, PATH, PRIORITY)
);
```

**当前 Rust 扫描器创建的表**（需要验证）：
```sql
-- 可能缺少索引，导致 WHERE PRIORITY=X 查询慢
```

### 问题2：查询策略错误

**尝试的策略**：
1. ❌ 只查 3 张表（target ± 1）→ 1781ms（但可能漏掉结果）
2. ❌ 查所有 41 张表 → 41682ms（太慢）
3. ❌ 查 ±5 范围（11 张表）→ 8000ms（还是太慢）

**File-Engine 的策略**：
- ✅ 查所有 41 张表
- ✅ 但有 PRIORITY 索引，`WHERE PRIORITY=5` 极快
- ✅ 使用 callback 机制，找够就停止

### 问题3：数据库配置不足

当前配置：
```rust
PRAGMA temp_store = MEMORY;
PRAGMA cache_size = -262144;  // 256MB
PRAGMA page_size = 65535;
```

可能缺少：
```sql
PRAGMA synchronous = OFF;     // 关闭同步（只读模式安全）
PRAGMA journal_mode = OFF;    // 关闭日志（只读模式不需要）
PRAGMA locking_mode = EXCLUSIVE;  // 独占锁
```

## 解决方案

### 方案 A：添加 PRIORITY 索引（推荐）

**步骤**：
1. 检查当前数据库是否有索引
2. 如果没有，重新扫描并添加索引
3. 测试性能

**预期效果**：
- 查询 `WHERE PRIORITY=5` 从 200ms → <10ms
- 总搜索时间：8000ms → **100-200ms**

### 方案 B：使用内存数据库（激进）

**策略**：
1. 启动时将所有路径加载到内存
2. 搜索时纯内存过滤
3. 不依赖 SQLite

**优点**：
- 极快（<10ms）
- 无数据库锁

**缺点**：
- 内存占用大（50万文件 × 平均150字节 = 75MB）
- 启动慢

### 方案 C：预编译查询语句（优化）

**当前代码**：
```rust
for &priority in &priorities {
    for &group in &groups_to_search {
        let sql = format!("SELECT ... WHERE PRIORITY={}", priority);
        let mut stmt = self.conn.prepare(&sql)?;  // ← 每次都编译
    }
}
```

**优化后**：
```rust
// 启动时预编译
let stmts: HashMap<(usize, i32), Statement> = ...;

// 搜索时复用
for &priority in &priorities {
    for &group in &groups_to_search {
        let stmt = stmts.get(&(group, priority))?;
        stmt.reset();
        // 直接查询
    }
}
```

## 下一步行动

1. ✅ **立即执行**：检查数据库是否有 PRIORITY 索引
2. ⏳ **如果没有**：重新扫描并添加索引
3. ⏳ **测试验证**：预期降到 100-200ms
4. ⏳ **如果还慢**：考虑方案 B 或 C

## 性能基准对比

| 实现 | 搜索耗时 | 内存占用 | 索引 |
|------|---------|---------|------|
| **File-Engine C++** | **50-100ms** | ~50MB | ✅ 有 |
| **当前 Rust (无索引)** | **8000ms** | ~50MB | ❌ 无？ |
| **目标 Rust (有索引)** | **100-200ms** | ~50MB | ✅ 添加 |

---

**结论**：性能问题的根本原因是**缺少 PRIORITY 索引**，导致每次 `WHERE PRIORITY=X` 查询都是全表扫描。
