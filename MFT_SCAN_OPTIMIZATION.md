# MFT 扫描性能优化方案

## 当前性能基线

### 扫描耗时（首次完整扫描）
| 驱动器 | 文件数 | 扫描耗时 | 数据库大小 |
|--------|--------|----------|------------|
| C 盘   | ~1.8M  | 50.40s   | 388 MB     |
| D 盘   | ~2.2M  | 63.21s   | 439 MB     |
| E 盘   | ~0.2M  | 3.77s    | 49 MB      |
| **总计（并行）** | **~4.2M** | **63.78s** ⚡ | **876 MB** |

**✅ 并行扫描状态**: 已启用  
**✅ 性能提升**: 1.84x（相比串行 117.5s）

### 性能评估
- **状态**: Good（良好）
- **目标**: < 60s（优秀）
- **改进空间**: 约 **6% 提升空间**（已接近目标）

---

## 性能瓶颈分析

### 1. 当前架构
```rust
// 顺序扫描（Sequential Scanning）
for drive in drives {
    scan_drive(drive);  // 阻塞，平均 ~40秒/盘
}
```

**问题**：
- 串行执行，无法利用多核 CPU
- I/O 等待时间浪费
- 总耗时 = Σ单个驱动器耗时

### 2. 耗时分布
根据日志分析：
- **MFT 读取**: ~40-50%（USN Journal 查询）
- **数据库写入**: ~30-40%（FTS5 批量插入）
- **文件名提取**: ~10-20%（UTF-16 转换）

---

## 优化方案

### 方案 1：并行扫描多驱动器 ✅ **已完成**

**实现难度**: 中等  
**实际提升**: 1.84x（63.78s vs 117.5s）  
**状态**: ✅ 已实现并验证

#### 当前实现
```rust
// lib.rs - 已实现并行扫描
let handles: Vec<_> = drives
    .iter()
    .map(|&drive| {
        std::thread::spawn(move || {
            let mut scanner = mft_scanner::UsnScanner::new(drive);
            scanner.scan_to_database(&output_dir, &config)
        })
    })
    .collect();

// 等待所有扫描完成
for handle in handles {
    handle.join();
}
```

#### 性能数据
| 驱动器 | 串行耗时 | 并行耗时 | 提升 |
|--------|----------|----------|------|
| C 盘   | 50.40s   | 50.40s   | -    |
| D 盘   | 63.21s   | 63.21s   | -    |
| E 盘   | 3.77s    | 3.77s    | -    |
| **总计** | **117.38s** | **63.78s** | **1.84x** ⚡ |

#### 优势
- ✅ 充分利用多核 CPU
- ✅ I/O 并行，减少等待时间
- ✅ 实现简单，代码已稳定

#### 实测效果
- ✅ 总耗时 = max(各驱动器耗时)
- ✅ 无磁盘 I/O 竞争问题（SSD）
- ✅ 内存占用正常（< 500MB）

---

### 方案 2：增量更新（Incremental Scan） ⭐⭐⭐⭐⭐

**实现难度**: 困难  
**预期提升**: 10-100x（后续扫描降至 ~1-5s）

#### 技术方案
```rust
// 首次扫描：完整扫描
fn full_scan(drive: char) -> Result<Vec<MftFileEntry>> {
    let scanner = Scanner::new(drive)?;
    let entries = scanner.scan()?;
    
    // 保存 USN Journal ID
    let journal_id = scanner.get_journal_id()?;
    save_journal_id(drive, journal_id)?;
    
    Ok(entries)
}

// 后续扫描：只查询变化
fn incremental_scan(drive: char) -> Result<Vec<MftFileEntry>> {
    let last_journal_id = load_journal_id(drive)?;
    let scanner = Scanner::new(drive)?;
    
    // 🔥 只查询自上次以来的变化
    let changes = scanner.scan_changes_since(last_journal_id)?;
    
    // 更新数据库
    for change in changes {
        match change.reason {
            ChangeReason::Created => db.insert(&change.entry)?,
            ChangeReason::Deleted => db.delete(&change.entry)?,
            ChangeReason::Modified => db.update(&change.entry)?,
            ChangeReason::Renamed => db.rename(&change.old_path, &change.new_path)?,
        }
    }
    
    Ok(changes)
}
```

#### 优势
- ✅ 极大减少后续扫描时间（1-5s vs 120s）
- ✅ 减少 CPU 和 I/O 负载
- ✅ 实时性更好

#### 实现步骤
1. 保存 USN Journal ID（首次扫描后）
2. 实现增量查询（`scan_changes_since`）
3. 实现数据库增删改操作（目前只有 `insert`）
4. 添加变化监控（实时更新）

---

### 方案 3：优化数据库写入 ⭐⭐

**实现难度**: 简单  
**预期提升**: 10-20%（总耗时降至 ~100s）

#### 当前优化状态
- ✅ 批量事务（`BEGIN ... COMMIT`）
- ✅ 关闭 journal_mode（`PRAGMA journal_mode = OFF`）
- ✅ FTS5 虚拟表（倒排索引）
- ✅ 内存缓存（`PRAGMA cache_size = -262144`）

#### 进一步优化
```rust
// 1. 增大批量大小
const BATCH_SIZE: usize = 10000;  // 当前可能较小

// 2. 使用内存数据库作为缓冲
let temp_db = Connection::open_in_memory()?;
// ... 写入内存数据库
// 最后一次性导出到磁盘

// 3. 并行写入不同表（如果有多个表）
```

---

### 方案 4：Memory-Mapped Files (mmap) ⭐

**实现难度**: 困难  
**预期提升**: 5-10%（MFT 读取优化）

#### 技术方案
```rust
use memmap2::Mmap;

// 使用内存映射读取 MFT
let file = File::open("\\\\.\\C:")?;
let mmap = unsafe { Mmap::map(&file)? };

// 直接从内存读取，减少系统调用
let entry = parse_mft_entry(&mmap[offset..]);
```

#### 优势
- ✅ 减少系统调用
- ✅ 利用操作系统页面缓存

#### 风险
- ⚠️ 内存占用大（MFT 可能数百 MB）
- ⚠️ 实现复杂度高

---

## 推荐优化路线

### Phase 1：并行扫描 ✅ **已完成**
1. **多线程并行扫描** - 已实现 1.84x 提升
   - ✅ 使用 std::thread 实现并行
   - ✅ 每个驱动器独立线程
   - ✅ 测试验证性能（63.78s）

### Phase 2：进一步优化（下一步） 🎯
2. **数据库写入优化** - 预期提升 10-20%
   - 当前批量大小：100,000
   - 优化方向：更大批量、内存缓冲
   
3. **单盘扫描优化** - 预期提升 10-15%
   - 优化 FRN 映射构建
   - 优化路径重建算法

### Phase 3：长期优化（1-2 周）
4. **增量更新** - 预期提升 10-100x（后续扫描）
   - 保存 USN Journal ID
   - 实现增量查询
   - 实现数据库增删改

---

## 性能目标

| 阶段 | 优化措施 | 目标耗时 | 相比基线 | 状态 |
|------|----------|----------|----------|------|
| **串行基线** | - | 117.5s | - | ❌ |
| **Phase 1** | 并行扫描 | 63.78s | **1.84x** ⚡ | ✅ **已完成** |
| **Phase 2** | 单盘+DB优化 | 50-55s | **2.1-2.3x** | ⏳ 进行中 |
| **Phase 3** | 增量更新 | 1-5s (后续) | **20-100x** 🚀 | 📋 规划中 |
| **最终目标** | 综合优化 | 45-50s (首次)<br>0.5-2s (后续) | **2.3-2.6x / 60-200x** 🔥 | 🎯 |

---

## 实现优先级

### P0 (立即实施)
- [x] FTS5 全文搜索（已完成）
- [x] 批量事务优化（已完成）
- [ ] **并行扫描** ⬅️ **下一步重点**

### P1 (短期规划)
- [ ] 增量更新
- [ ] 数据库增删改操作

### P2 (长期规划)
- [ ] Memory-Mapped Files
- [ ] 进一步数据库优化

---

## 测试验证

### 性能测试脚本
```powershell
# 分析当前性能
.\analyze_scan_perf.ps1

# 测试优化后性能
.\test_mft_scan_perf.ps1
```

### 验收标准
- ✅ 首次扫描 < 60s
- ✅ 后续扫描 < 5s（实现增量更新后）
- ✅ 数据完整性 100%
- ✅ 内存占用 < 500MB
