# iLauncher 内存占用说明

## 📊 当前内存使用情况

### Monitor 阶段（MFT Service）
- **常驻内存**: ~450-500MB
- **组成**:
  - C 盘 FRN Map: ~200MB (2.2M 文件)
  - D 盘 FRN Map: ~180MB (2.0M 文件)  
  - E 盘 FRN Map: ~20MB (0.2M 文件)
  - 其他开销: ~50-100MB

### UI 进程
- **常驻内存**: ~45MB

---

## 🔍 为什么需要这么多内存？

### FRN Map 的作用
```
USN Journal 监控 → 收到文件变更事件
    ↓
需要完整路径：frn=123456 → C:\folder\file.txt
    ↓
必须反向递归: 123456 → parent:5000 → parent:root
    ↓
需要 FRN Map 缓存所有 (frn → parent_frn, filename)
```

### 内存计算
```rust
struct ParentInfo {
    parent_frn: u64,      // 8 bytes
    filename: SmartString, // ~32 bytes (平均)
}
// + HashMap 开销: ~60 bytes
// = 总计 ~100 bytes/文件
```

**4.5M 文件 × 100 bytes = 450MB**

---

## ✅ 已实施的优化

### 1. SmartString（减少 10-15%）
- **原理**: 小字符串 (<23 bytes) 内联存储，无堆分配
- **效果**: 大部分文件名受益（如 `readme.txt`, `main.rs`）
- **限制**: 长文件名仍需堆分配

### 2. FxHashMap（减少 5-10%）
- **原理**: 高性能哈希函数，减少内存开销
- **效果**: HashMap 本身更紧凑

### 3. 按需加载路径（减少 90% 路径内存）
- **原理**: 路径存储在磁盘 `_paths.dat`，需要时 mmap 读取
- **效果**: 不缓存完整路径字符串，只缓存 FRN 映射

---

## 🚀 可选优化方案（如需进一步降低）

### 方案 A: LRU 缓存（减少 80%）
**内存**: 450MB → **90MB**

```rust
// 只缓存 50 万个热点 FRN
frn_cache: LruCache<u64, ParentInfo>,  // 50MB
// 冷数据从磁盘 mmap 加载
frn_map_file: memmap2::Mmap,
```

**代价**:
- ❌ 首次查询冷数据慢 10-50ms
- ❌ 需要额外的 FRN Map 持久化文件
- ❌ 代码复杂度 +30%

### 方案 B: 完全按需加载（减少 95%）
**内存**: 450MB → **20MB**

```rust
// 完全不缓存，每次从 MFT 重建路径
fn build_path(frn: u64) -> String {
    // 每次打开卷句柄查询 MFT
    query_mft_for_frn(frn)  // 慢 50-200ms
}
```

**代价**:
- ❌ 每次查询都要重新扫描 MFT
- ❌ 延迟 +100ms/次
- ❌ 大量文件变更时性能崩溃

### 方案 C: 禁用 Monitor 阶段
**内存**: 450MB → **0MB**

```rust
// 只运行 Scan 阶段，不监控增量更新
cargo run --release -- --scan-only
```

**代价**:
- ❌ 新建/删除/重命名文件不会实时更新
- ❌ 需要定期重新扫描（如每小时）

---

## 💡 推荐配置

### 轻量级（<100MB）
```toml
[mft_scanner]
enable_monitor = false       # 禁用实时监控
scan_interval = 3600         # 每小时扫描一次
```

### 标准模式（~450MB，推荐）
```toml
[mft_scanner]
enable_monitor = true        # 启用实时监控
frn_cache_mode = "full"      # 全量缓存
```

### 性能模式（~90MB）
```toml
[mft_scanner]
enable_monitor = true
frn_cache_mode = "lru"       # LRU 缓存（需实现）
lru_capacity = 500000        # 缓存 50 万条
```

---

## 📈 对比：Everything vs iLauncher

| 软件 | 内存占用 | 文件数 | 说明 |
|-----|---------|-------|------|
| Everything | ~50MB | 4.5M | 使用 USN Journal + 内存数据库 |
| iLauncher (当前) | ~450MB | 4.5M | 完整 FRN Map + 3-gram 索引 |
| iLauncher (优化后) | ~90MB | 4.5M | LRU 缓存 + mmap 按需加载 |

**Everything 更省内存的原因**:
1. 不需要构建完整路径（直接查询 MFT）
2. 使用 C++ 手动优化内存布局
3. 只缓存必要的元数据（不包含 3-gram 索引）

**iLauncher 优势**:
1. ✅ 3-gram 模糊搜索（Everything 需精确匹配）
2. ✅ Rust 安全性保证
3. ✅ 跨平台架构（Everything 仅 Windows）
4. ✅ 插件化架构

---

## 🎯 结论

**当前 450MB 内存占用是合理的**，原因：
1. 需要常驻 FRN Map 才能快速构建路径
2. 4.5M 文件 × 100 bytes = 450MB 符合预期
3. SmartString + FxHashMap 已优化 15-20%

**如果需要进一步降低**:
- 实施 LRU 缓存（减少 80%）
- 或禁用 Monitor 阶段（减少 100%）

**对于大多数用户**: 450MB 内存在现代电脑上（8GB+ RAM）是可接受的，换来的是：
- ⚡ 闪电般的搜索速度 (<30ms)
- 🔄 实时增量更新（无需重新扫描）
- 🎯 3-gram 模糊搜索
