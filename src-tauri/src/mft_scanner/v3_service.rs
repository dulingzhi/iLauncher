// V3 索引服务 - 每盘一个线程：快照 + USN catch-up + compact 策略 + 启动守卫
//
// 生命周期（Phase 2 收尾，解决 C4"每次启动全量删库重建"）：
//   启动: 快照存在且水位有效 → LiveIndex::open + catch_up_volume（秒开）
//         快照缺失 / 水位失效（journal 重建）→ scan_mft_streaming_v3 全量重建
//   运行: 每 CATCH_UP_INTERVAL  catch-up 一次（replay journal 进 overlay）
//         pending 超阈值或距上次 compact 超间隔 → compact 折叠新快照
//   退出: 有 pending 变更 → 最终 compact（水位随 header 持久化，
//         下次启动即可从断点 catch-up）
//
// 与 v2 链路（UsnIncrementalUpdater + DeltaMerger）并行运行：
// UI 查询当前仍走 v2 索引文件，Phase 3 切换查询路径后 v2 退役。
//
// 可单测的部分（decide_startup / CompactPolicy）为纯函数；run() 仅做薄编排。

use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::{error, info, warn};

use crate::index_v2::usn_journal::WaterLevel;
use crate::index_v2::{CatchUpOutcome, LiveIndex};

use super::streaming_builder::StreamingBuilder;

/// catch-up 轮询间隔
pub const CATCH_UP_INTERVAL: Duration = Duration::from_secs(2);
/// 退出前最终 compact 的 pending 阈值（低于则不值得重写快照）
pub const FINAL_COMPACT_MIN_PENDING: usize = 1;

/// 启动守卫决策（纯函数）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupAction {
    /// 打开现有快照 + catch-up（冷启动秒开路径）
    OpenSnapshot,
    /// 快照缺失或水位失效 → 全量重建该盘
    Rebuild,
}

pub fn decide_startup(snapshot_exists: bool, water: WaterLevel) -> StartupAction {
    if !snapshot_exists {
        return StartupAction::Rebuild;
    }
    match water {
        WaterLevel::Valid | WaterLevel::NoWaterLevel => StartupAction::OpenSnapshot,
        WaterLevel::JournalRecreated => StartupAction::Rebuild,
    }
}

/// compact 策略（纯函数，可单测）
#[derive(Debug, Clone)]
pub struct CompactPolicy {
    /// overlay 待折叠规模阈值
    pub pending_threshold: usize,
    /// 距上次 compact 的最小间隔
    pub min_interval: Duration,
}

impl Default for CompactPolicy {
    fn default() -> Self {
        Self {
            pending_threshold: 100_000,
            min_interval: Duration::from_secs(30 * 60),
        }
    }
}

impl CompactPolicy {
    /// 到达 compact 时机？
    /// pending 超阈值且距上次 compact 超过最小间隔。
    pub fn should_compact(&self, pending: usize, since_last: Duration) -> bool {
        pending >= self.pending_threshold && since_last >= self.min_interval
    }
}

/// 单盘 V3 索引服务
pub struct V3DriveService {
    drive: char,
    output_dir: String,
    policy: CompactPolicy,
}

impl V3DriveService {
    pub fn new(drive: char, output_dir: String) -> Self {
        Self {
            drive,
            output_dir,
            policy: CompactPolicy::default(),
        }
    }

    fn snapshot_path(&self) -> PathBuf {
        PathBuf::from(&self.output_dir).join(format!("{}.snapshot", self.drive))
    }

    /// 全量重建（v3 快照直出，header 携带 USN 水位）
    fn rebuild(&self) -> Result<()> {
        info!("🔄 [v3] Drive {}: full rebuild (v3 snapshot)", self.drive);
        let mut builder = StreamingBuilder::new(self.drive, &self.output_dir)?;
        builder.scan_mft_streaming_v3(&self.output_dir)?;
        Ok(())
    }

    /// 启动：加载或重建，返回可用的 LiveIndex
    fn open_or_rebuild(&self) -> Result<LiveIndex> {
        let path = self.snapshot_path();
        let snapshot_exists = path.exists();

        if snapshot_exists {
            match LiveIndex::open(&path) {
                Ok(index) => {
                    info!(
                        "✓ [v3] Drive {}: snapshot opened ({} rows, water journal={:#X} next_usn={})",
                        self.drive,
                        index.snapshot().row_count(),
                        index.journal_id(),
                        index.next_usn()
                    );
                    return Ok(index);
                }
                Err(e) => {
                    warn!("⚠️  [v3] Drive {}: snapshot open failed, rebuilding: {:#}", self.drive, e);
                }
            }
        }

        self.rebuild()?;
        // 重建后必然可打开（同进程刚写完）
        LiveIndex::open(&path).map_err(|e| {
            anyhow::anyhow!("[v3] Drive {}: rebuilt snapshot still unreadable: {:#}", self.drive, e)
        })
    }

    /// 阻塞式运行，直到 running 置 false
    pub fn run(&self, running: Arc<AtomicBool>) -> Result<()> {
        info!("👀 [v3] Starting v3 index service for drive {}", self.drive);

        let path = self.snapshot_path();
        let mut index = self.open_or_rebuild()?;
        let mut last_compact = Instant::now();

        // 启动时先追一次增量（快照冻结至今的变更）
        match index.catch_up_volume(self.drive) {
            Ok(CatchUpOutcome::CaughtUp(n)) => info!("✓ [v3] Drive {}: caught up {} changes", self.drive, n),
            Ok(CatchUpOutcome::BaselineSet(usn)) => {
                info!("✓ [v3] Drive {}: no water level, baseline set at usn {}", self.drive, usn)
            }
            Ok(CatchUpOutcome::RebuildNeeded) => {
                warn!("⚠️  [v3] Drive {}: journal recreated at startup, rebuilding", self.drive);
                self.rebuild()?;
                index = LiveIndex::open(&path)?;
            }
            Err(e) => warn!("⚠️  [v3] Drive {}: startup catch-up failed (non-fatal): {:#}", self.drive, e),
        }

        while running.load(Ordering::SeqCst) {
            std::thread::sleep(CATCH_UP_INTERVAL);
            if !running.load(Ordering::SeqCst) {
                break;
            }

            match index.catch_up_volume(self.drive) {
                Ok(CatchUpOutcome::CaughtUp(n)) => {
                    if n > 0 {
                        tracing::debug!("[v3] Drive {}: +{} changes", self.drive, n);
                    }
                }
                Ok(CatchUpOutcome::RebuildNeeded) => {
                    error!("❌ [v3] Drive {}: journal recreated mid-run, full rebuild", self.drive);
                    if let Err(e) = self.rebuild() {
                        error!("❌ [v3] Drive {}: rebuild failed: {:#}", self.drive, e);
                    } else {
                        match LiveIndex::open(&path) {
                            Ok(fresh) => {
                                index = fresh;
                                last_compact = Instant::now();
                            }
                            Err(e) => error!("❌ [v3] Drive {}: reopen after rebuild failed: {:#}", self.drive, e),
                        }
                    }
                }
                Ok(CatchUpOutcome::BaselineSet(usn)) => {
                    warn!("⚠️  [v3] Drive {}: unexpected baseline reset at usn {}", self.drive, usn);
                }
                Err(e) => warn!("⚠️  [v3] Drive {}: catch-up failed (transient): {:#}", self.drive, e),
            }

            // compact 策略
            let pending = index.overlay().pending_len();
            if self.policy.should_compact(pending, last_compact.elapsed()) {
                info!("🗜️  [v3] Drive {}: compacting (pending={})", self.drive, pending);
                match index.compact(&path) {
                    Ok(()) => last_compact = Instant::now(),
                    Err(e) => error!("❌ [v3] Drive {}: compact failed: {:#}", self.drive, e),
                }
            }
        }

        // 退出前最终 compact：水位随 header 持久化，下次启动从断点 catch-up
        let pending = index.overlay().pending_len();
        if pending >= FINAL_COMPACT_MIN_PENDING {
            info!("🛑 [v3] Drive {}: final compact on shutdown (pending={})", self.drive, pending);
            if let Err(e) = index.compact(&path) {
                warn!("⚠️  [v3] Drive {}: final compact failed (water level not persisted): {:#}", self.drive, e);
            }
        } else {
            info!("✓ [v3] Drive {}: shutdown, no pending changes (water already persisted)", self.drive);
        }

        info!("✓ [v3] Drive {}: service stopped", self.drive);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decide_startup() {
        use crate::index_v2::usn_journal::WaterLevel::*;
        assert_eq!(decide_startup(false, Valid), StartupAction::Rebuild);
        assert_eq!(decide_startup(false, NoWaterLevel), StartupAction::Rebuild);
        assert_eq!(decide_startup(true, Valid), StartupAction::OpenSnapshot);
        assert_eq!(decide_startup(true, NoWaterLevel), StartupAction::OpenSnapshot);
        assert_eq!(decide_startup(true, JournalRecreated), StartupAction::Rebuild);
    }

    #[test]
    fn test_compact_policy_threshold_and_interval() {
        let policy = CompactPolicy::default();

        // 未达阈值：不 compact
        assert!(!policy.should_compact(999, Duration::from_secs(3600)));
        // 达阈值但间隔不足：不 compact
        assert!(!policy.should_compact(100_000, Duration::from_secs(60)));
        // 达阈值且间隔足够：compact
        assert!(policy.should_compact(100_000, Duration::from_secs(1800)));
        // 边界：恰在阈值
        assert!(policy.should_compact(100_000, Duration::from_secs(30 * 60)));

        let aggressive = CompactPolicy {
            pending_threshold: 10,
            min_interval: Duration::from_secs(0),
        };
        assert!(aggressive.should_compact(10, Duration::from_secs(0)));
    }
}
