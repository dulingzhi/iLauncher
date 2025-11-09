# 数据库选型和性能优化方案

## 当前状态

- **数据库**: SQLite 3
- **数据量**: 约 50 万条文件记录（3 个驱动器）
- **搜索性能**: 200-400ms
- **目标**: < 100ms（File-Engine 水平）

---

## 数据库性能对比

### 1. SQLite（当前方案）✅

**优点**:
- ✅ 文件型数据库，无需服务进程
- ✅ 跨平台，稳定可靠
- ✅ Rust 生态成熟（rusqlite）
- ✅ File-Engine 也用 SQLite

**缺点**:
- ⚠️ 全文搜索需要手动实现（内存过滤）
- ⚠️ 读取性能受磁盘 I/O 限制
- ⚠️ 无法利用 CPU 向量化指令

**当前性能**: 200-400ms

---

### 2. RocksDB（推荐）⭐⭐⭐⭐⭐

**简介**: Facebook 开发的高性能键值存储引擎

**优点**:
- ⚡ **极快的读取性能**（LSM Tree 结构）
- ⚡ **内存缓存**（Block Cache）
- ⚡ **压缩存储**（节省 30-50% 磁盘空间）
- ✅ Rust 绑定成熟（`rust-rocksdb`）
- ✅ 支持前缀搜索（Prefix Bloom Filter）

**缺点**:
- ❌ 写入性能略低于 SQLite（对我们影响不大）
- ❌ 学习曲线略高

**预期性能**: **50-100ms** ⚡⚡⚡

**使用方案**:
```rust
// Key: 文件名小写 + 路径哈希
// Value: 序列化的 MftFileEntry

db.put(b"chrome.exe#hash123", bincode::serialize(&entry)?)?;

// 前缀搜索
let iter = db.prefix_iterator(b"chrome");
for (key, value) in iter {
    // 解析结果
}
```

---

### 3. sled（纯 Rust）⭐⭐⭐⭐

**简介**: 纯 Rust 实现的嵌入式数据库

**优点**:
- ⚡ 纯 Rust，零 FFI 开销
- ⚡ 支持事务
- ⚡ 自动压缩
- ✅ API 简洁

**缺点**:
- ⚠️ 生态不如 RocksDB 成熟
- ⚠️ 性能略低于 RocksDB

**预期性能**: **100-150ms** ⚡⚡

---

### 4. Tantivy（全文搜索引擎）⭐⭐⭐⭐⭐

**简介**: Rust 实现的全文搜索引擎（类似 Elasticsearch）

**优点**:
- ⚡⚡⚡ **专为全文搜索优化**
- ⚡ **倒排索引**（Inverted Index）
- ⚡ **模糊搜索**、拼音搜索
- ⚡ **高亮显示**、相关性排序
- ✅ 纯 Rust，性能极佳

**缺点**:
- ❌ 索引体积大（可能是原始数据的 2-3 倍）
- ❌ 写入需要重建索引

**预期性能**: **10-50ms** ⚡⚡⚡⚡⚡

**使用方案**:
```rust
// 创建索引
let mut index_writer = index.writer(50_000_000)?;
let doc = doc!(
    path => "C:\\Program Files\\chrome.exe",
    filename => "chrome.exe",
    priority => 5,
);
index_writer.add_document(doc)?;

// 搜索
let query_parser = QueryParser::for_index(&index, vec![filename]);
let query = query_parser.parse_query("chrome")?;
let results = searcher.search(&query, &TopDocs::with_limit(50))?;
```

---

### 5. 内存数据库（极端方案）⭐⭐⭐

**方案**: 启动时加载所有文件路径到内存

**优点**:
- ⚡⚡⚡ **极快搜索**（< 10ms）
- ✅ 无数据库 I/O

**缺点**:
- ❌ 内存占用大（50 万文件 × 150 字节 = 75MB）
- ❌ 启动慢（需要加载所有数据）
- ❌ 更新麻烦（需要重新加载）

**预期性能**: **5-20ms** ⚡⚡⚡⚡⚡

**数据结构**:
```rust
// 使用 HashMap + Vec
struct MemoryIndex {
    // 文件名 → 路径列表
    name_map: HashMap<String, Vec<MftFileEntry>>,
    // 路径前缀树（用于前缀搜索）
    trie: PathTrie,
}
```

---

## 性能优化方案对比

| 方案 | 搜索时间 | 内存占用 | 索引大小 | 实现难度 | 推荐度 |
|------|---------|---------|---------|---------|--------|
| **SQLite（当前）** | 200-400ms | ~50MB | ~200MB | 低 | ⭐⭐⭐ |
| **RocksDB** | **50-100ms** | ~100MB | ~150MB | 中 | ⭐⭐⭐⭐⭐ |
| **sled** | 100-150ms | ~80MB | ~180MB | 中 | ⭐⭐⭐⭐ |
| **Tantivy** | **10-50ms** | ~150MB | ~400MB | 高 | ⭐⭐⭐⭐⭐ |
| **内存索引** | **5-20ms** | ~200MB | 0 | 中 | ⭐⭐⭐ |

---

## 推荐方案

### 方案 A：RocksDB（最佳平衡）⭐⭐⭐⭐⭐

**适用场景**: 需要高性能 + 低内存占用

**实现步骤**:
1. 添加依赖：`rocksdb = "0.21"`
2. 扫描时写入：
   ```rust
   let db = DB::open_default("mft_index.db")?;
   let key = format!("{}#{}", filename.to_lowercase(), hash);
   db.put(key, bincode::serialize(&entry)?)?;
   ```
3. 搜索时读取：
   ```rust
   let iter = db.prefix_iterator(query.to_lowercase().as_bytes());
   for (key, value) in iter.take(50) {
       let entry: MftFileEntry = bincode::deserialize(&value)?;
       results.push(entry);
   }
   ```

**预期效果**: **50-100ms** ⚡⚡⚡

---

### 方案 B：Tantivy（极致性能）⭐⭐⭐⭐⭐

**适用场景**: 需要极致搜索性能 + 模糊搜索

**实现步骤**:
1. 添加依赖：`tantivy = "0.21"`
2. 创建索引：
   ```rust
   let schema = Schema::builder()
       .add_text_field("path", TEXT | STORED)
       .add_text_field("filename", TEXT)
       .add_i64_field("priority", INDEXED)
       .build();
   
   let index = Index::create_in_dir("mft_index", schema)?;
   ```
3. 搜索：
   ```rust
   let searcher = reader.searcher();
   let query = QueryParser::parse_query("chrome")?;
   let top_docs = searcher.search(&query, &TopDocs::with_limit(50))?;
   ```

**预期效果**: **10-50ms** ⚡⚡⚡⚡⚡

---

### 方案 C：混合方案（保守优化）⭐⭐⭐⭐

**策略**: SQLite（持久化）+ 内存缓存（热数据）

**实现步骤**:
1. 启动时加载常用文件（.exe, .lnk）到内存
2. 搜索时：
   - 先查内存缓存（< 10ms）
   - 未命中再查 SQLite（200ms）
3. LRU 淘汰策略（保持内存在 100MB 以内）

**预期效果**: 
- 常见程序：**10-20ms** ⚡⚡⚡⚡
- 其他文件：**200ms** ⚡

---

## 其他优化点

### 1. 使用 FTS5（SQLite 全文搜索扩展）

```sql
-- 创建虚拟表
CREATE VIRTUAL TABLE files_fts USING fts5(path, filename, priority);

-- 插入数据
INSERT INTO files_fts VALUES ('C:\chrome.exe', 'chrome.exe', 5);

-- 搜索（10倍提升！）
SELECT * FROM files_fts WHERE filename MATCH 'chrome' LIMIT 50;
```

**预期效果**: **50-100ms** ⚡⚡⚡

**优点**:
- ✅ 无需更换数据库
- ✅ 原生支持全文搜索
- ✅ 实现简单

**缺点**:
- ⚠️ 索引体积大（+50%）
- ⚠️ 需要重新扫描

---

### 2. SIMD 优化（字符串匹配）

使用 `memchr` 或 `aho-corasick` 加速字符串匹配：

```rust
use memchr::memmem;

// 比 contains() 快 2-3 倍
let finder = memmem::Finder::new(query.as_bytes());
if finder.find(path.as_bytes()).is_some() {
    // 匹配成功
}
```

**预期效果**: **10-20% 提升**

---

### 3. 预编译正则表达式

如果支持正则搜索：

```rust
use regex::Regex;
use once_cell::sync::Lazy;

static SEARCH_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"chrome.*\.exe").unwrap()
});

if SEARCH_REGEX.is_match(&path) {
    // 匹配
}
```

---

## 总结建议

### 立即可做（低成本，高收益）

1. ✅ **启用 SQLite FTS5**
   - 耗时：1-2 小时
   - 预期提升：**2-4 倍**（200ms → 50-100ms）

2. ✅ **添加内存缓存**（LRU）
   - 耗时：2-3 小时
   - 预期提升：常见程序 **10 倍**（200ms → 20ms）

### 中期迁移（高收益）

3. ⏳ **迁移到 RocksDB**
   - 耗时：1-2 天
   - 预期提升：**3-4 倍**（200ms → 50-100ms）
   - 长期收益高

### 极致优化（可选）

4. ⏳ **迁移到 Tantivy**
   - 耗时：3-5 天
   - 预期提升：**10-20 倍**（200ms → 10-50ms）
   - 适合追求极致性能

---

## 我的推荐

**优先级 1**: SQLite FTS5（性价比最高）
**优先级 2**: 内存缓存 LRU（对常见搜索极有效）
**优先级 3**: RocksDB（如果需要进一步优化）

需要我帮你实现哪个方案？
