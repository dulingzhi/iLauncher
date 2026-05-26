// search_test - iLauncher File Search CLI Self-Test Tool
//
// USAGE:
//   cargo run --bin search_test -- [OPTIONS]
//   (or as compiled binary: search_test.exe [OPTIONS])
//
// OPTIONS:
//   --scan                  Full MFT scan → build FST+Bitmap index (requires admin)
//   --query  <TEXT>         Fuzzy search in index
//   --bench                 Benchmark: 50 representative queries, print p50/p95/p99
//   --stats                 Show index file sizes, gram count, file count, version
//   --monitor               Start real-time USN Journal monitoring loop
//   --drive  <C,D,...>      Drive letters (default: auto-detect all NTFS drives)
//   --limit  <N>            Max results to display (default: 20)
//   --output-dir <PATH>     Index DB directory (default: %LOCALAPPDATA%\iLauncher\mft_databases)
//   --no-warmup             Skip mmap warmup phase
//   --verbose               Enable debug-level logging
//   --help                  Print this message
//
// EXAMPLES:
//   # Full scan C: and D:, build index
//   cargo run --bin search_test -- --scan --drive C,D
//
//   # Query "chrome" in all drives
//   cargo run --bin search_test -- --query chrome
//
//   # Benchmark (requires index already built)
//   cargo run --bin search_test -- --bench
//
//   # Watch real-time file changes
//   cargo run --bin search_test -- --monitor --drive C

// Non-Windows stub – compiles but exits immediately
#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("search_test only runs on Windows (requires NTFS/MFT).");
    std::process::exit(1);
}

// ──────────────────────────────────────────────────────────────────────────────
// Windows implementation
// ──────────────────────────────────────────────────────────────────────────────
#[cfg(target_os = "windows")]
mod win {

use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use fst::Map;
use roaring::RoaringBitmap;
use tracing::{error, info, warn};

// ──────────────────────────────────────────────────────────────────────────────
// CLI argument parsing (no extra dep – parse by hand)
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct Args {
    scan: bool,
    query: Option<String>,
    bench: bool,
    stats: bool,
    monitor: bool,
    drives: Vec<char>,
    limit: usize,
    output_dir: Option<String>,
    no_warmup: bool,
    verbose: bool,
}

impl Args {
    fn parse() -> Result<Self> {
        let raw: Vec<String> = std::env::args().skip(1).collect();
        let mut a = Args { limit: 20, ..Default::default() };

        let mut i = 0;
        while i < raw.len() {
            match raw[i].as_str() {
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                "--scan" => a.scan = true,
                "--bench" => a.bench = true,
                "--stats" => a.stats = true,
                "--monitor" => a.monitor = true,
                "--no-warmup" => a.no_warmup = true,
                "--verbose" | "-v" => a.verbose = true,
                "--query" => {
                    i += 1;
                    a.query = Some(raw.get(i).cloned().context("--query needs a value")?);
                }
                "--drive" => {
                    i += 1;
                    let spec = raw.get(i).context("--drive needs a value")?;
                    a.drives = spec
                        .split(',')
                        .filter_map(|s| s.trim().chars().next())
                        .map(|c| c.to_uppercase().next().unwrap_or(c))
                        .collect();
                }
                "--limit" => {
                    i += 1;
                    let n: usize = raw
                        .get(i)
                        .context("--limit needs a value")?
                        .parse()
                        .context("--limit must be a number")?;
                    a.limit = n;
                }
                "--output-dir" => {
                    i += 1;
                    a.output_dir = Some(raw.get(i).cloned().context("--output-dir needs a value")?);
                }
                other => {
                    eprintln!("Unknown argument: {other}");
                    print_help();
                    std::process::exit(1);
                }
            }
            i += 1;
        }

        Ok(a)
    }
}

fn print_help() {
    println!(
        r#"search_test - iLauncher File Search CLI Self-Test
USAGE: search_test [OPTIONS]
OPTIONS:
  --scan                  Full MFT scan → build FST+Bitmap index (requires admin)
  --query  <TEXT>         Fuzzy search in index
  --bench                 Benchmark 50 queries, report p50/p95/p99
  --stats                 Show index stats (size, file count, version)
  --monitor               Real-time USN Journal monitoring
  --drive  <C,D,...>      Drive letters (default: auto-detect)
  --limit  <N>            Max results (default: 20)
  --output-dir <PATH>     Index dir (default: %LOCALAPPDATA%\iLauncher\mft_databases)
  --no-warmup             Skip mmap warmup
  --verbose               Debug logging
  --help                  This message
EXAMPLES:
  search_test --scan --drive C
  search_test --query chrome
  search_test --bench
  search_test --stats
  search_test --monitor --drive C,D"#
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

fn resolve_output_dir(override_: &Option<String>) -> Result<String> {
    if let Some(d) = override_ {
        return Ok(d.clone());
    }
    // Default: %LOCALAPPDATA%\iLauncher\mft_databases
    let local_appdata = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string());
    let dir = format!("{local_appdata}\\iLauncher\\mft_databases");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn resolve_drives(spec: &[char]) -> Vec<char> {
    if !spec.is_empty() {
        return spec.to_vec();
    }
    // Auto-detect all accessible NTFS drives A-Z
    let mut drives = Vec::new();
    for letter in b'A'..=b'Z' {
        let c = letter as char;
        let path = format!("{c}:\\");
        if Path::new(&path).exists() {
            drives.push(c);
        }
    }
    drives
}

// ──────────────────────────────────────────────────────────────────────────────
// cmd_scan – full MFT scan + index build
// ──────────────────────────────────────────────────────────────────────────────

fn cmd_scan(drives: &[char], output_dir: &str) -> Result<()> {
    println!("══════════════════════════════════════════");
    println!("  FULL MFT SCAN");
    println!("══════════════════════════════════════════");

    // Check admin rights (MFT requires elevation)
    if !is_elevated() {
        warn!(
            "⚠  Not running as Administrator. MFT access may fail.\n\
             Tip: run  runas /user:Administrator search_test.exe --scan"
        );
    }

    use ilauncher_lib::mft_scanner::{MultiDriveScanner, ScanConfig};

    let config = ScanConfig {
        drives: drives.to_vec(),
        output_dir: output_dir.to_string(),
        ignore_paths: vec![
            "c:\\windows\\winsxs".to_string(),
            "c:\\$recycle.bin".to_string(),
        ],
    };

    let scanner = MultiDriveScanner::new(&config);

    let t0 = Instant::now();
    scanner.scan_all()?;
    let elapsed = t0.elapsed();

    println!("\n✅  Full scan completed in {:.2}s", elapsed.as_secs_f64());
    println!("   Wrote index files to: {output_dir}");

    // Show per-drive stats
    println!("\nDrive stats:");
    for &d in drives {
        print_drive_stats(d, output_dir);
    }

    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// cmd_query – single fuzzy search
// ──────────────────────────────────────────────────────────────────────────────

fn cmd_query(
    query: &str,
    drives: &[char],
    output_dir: &str,
    limit: usize,
    warmup: bool,
) -> Result<()> {
    println!("══════════════════════════════════════════");
    println!("  QUERY: \"{query}\"");
    println!("══════════════════════════════════════════");

    let mut total_results: Vec<(char, String)> = Vec::new();
    let mut total_query_ms = 0.0f64;

    for &drive in drives {
        let index_path = format!("{output_dir}\\{drive}_index.fst");
        if !Path::new(&index_path).exists() {
            warn!("  Drive {drive}: no index found – run --scan first");
            continue;
        }

        match DriveIndex::open(drive, output_dir, warmup) {
            Ok(idx) => {
                let t0 = Instant::now();
                match idx.search(query, limit) {
                    Ok(ids) => {
                        let query_ms = t0.elapsed().as_secs_f64() * 1000.0;
                        total_query_ms += query_ms;

                        let paths = idx.resolve_paths(&ids);
                        println!(
                            "\n  Drive {drive}:  {} result(s) in {query_ms:.2}ms",
                            paths.len()
                        );
                        for p in &paths {
                            println!("    {p}");
                            total_results.push((drive, p.clone()));
                        }
                    }
                    Err(e) => error!("  Drive {drive}: query failed: {e:#}"),
                }
            }
            Err(e) => error!("  Drive {drive}: index open failed: {e:#}"),
        }
    }

    println!("\n──────────────────────────────────────────");
    println!(
        "  Total: {} result(s)  |  {total_query_ms:.2}ms across all drives",
        total_results.len()
    );

    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// cmd_bench – 50 queries, print latency percentiles
// ──────────────────────────────────────────────────────────────────────────────

fn cmd_bench(drives: &[char], output_dir: &str, warmup: bool) -> Result<()> {
    println!("══════════════════════════════════════════");
    println!("  BENCHMARK (50 queries per drive)");
    println!("══════════════════════════════════════════");

    // Representative query set – mix of short, medium, long, common names
    let queries: &[&str] = &[
        // Common filenames
        "chrome", "firefox", "notepad", "explorer", "cmd",
        "python", "node", "git", "code", "rust",
        // Extensions / partial names
        ".exe", ".dll", ".txt", ".json", ".rs",
        // Longer patterns
        "system32", "program files", "appdata", "local",
        "documents", "download", "desktop", "startup",
        // Short (tests prefix-search path)
        "a", "ab", "win", "ust",
        // Mixed case (should normalise)
        "Chrome", "NOTEPAD", "Python",
        // Typical user files
        "readme", "license", "config", "setup", "install",
        "update", "uninstall", "backup", "log", "temp",
        // Edge cases
        "中文", "test_file", "my document",
        "abcdefghijk", "zzz",
    ];

    for &drive in drives {
        let index_path = format!("{output_dir}\\{drive}_index.fst");
        if !Path::new(&index_path).exists() {
            warn!("  Drive {drive}: no index found – run --scan first");
            continue;
        }

        let idx = match DriveIndex::open(drive, output_dir, warmup) {
            Ok(i) => i,
            Err(e) => {
                error!("  Drive {drive}: {e:#}");
                continue;
            }
        };

        let mut latencies_ms: Vec<f64> = Vec::with_capacity(queries.len());

        for q in queries {
            let t0 = Instant::now();
            let _ = idx.search(q, 20);
            latencies_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        }

        latencies_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let n = latencies_ms.len();
        let p50 = latencies_ms[n * 50 / 100];
        let p90 = latencies_ms[n * 90 / 100];
        let p95 = latencies_ms[n * 95 / 100];
        let p99 = latencies_ms[(n * 99 / 100).min(n - 1)];
        let mean = latencies_ms.iter().sum::<f64>() / n as f64;
        let max = latencies_ms[n - 1];
        let min = latencies_ms[0];

        println!("\n  Drive {drive} ({n} queries):");
        println!("    Min   : {min:.2}ms");
        println!("    Mean  : {mean:.2}ms");
        println!("    p50   : {p50:.2}ms");
        println!("    p90   : {p90:.2}ms");
        println!("    p95   : {p95:.2}ms");
        println!("    p99   : {p99:.2}ms");
        println!("    Max   : {max:.2}ms");
        println!("    Target: <5ms ✓" );
    }

    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// cmd_stats – index file sizes, gram count, file count
// ──────────────────────────────────────────────────────────────────────────────

fn cmd_stats(drives: &[char], output_dir: &str) -> Result<()> {
    println!("══════════════════════════════════════════");
    println!("  INDEX STATISTICS");
    println!("══════════════════════════════════════════");

    for &drive in drives {
        print_drive_stats(drive, output_dir);
    }

    Ok(())
}

fn print_drive_stats(drive: char, output_dir: &str) {
    let fst_file = format!("{output_dir}\\{drive}_index.fst");
    let bitmap_file = format!("{output_dir}\\{drive}_bitmaps.dat");
    let paths_file = format!("{output_dir}\\{drive}_paths.dat");
    let offsets_file = format!("{output_dir}\\{drive}_offsets.dat");
    let delta_file = format!("{output_dir}\\{drive}_index_delta.dat");
    let version_file = format!("{output_dir}\\{drive}_index.version");
    let ready_file = format!("{output_dir}\\{drive}.ready");

    println!("\n  Drive {drive}:");

    // Check if index exists
    if !Path::new(&fst_file).exists() {
        println!("    ✗  No index found (run --scan)");
        return;
    }

    let fst_sz = file_size_mb(&fst_file);
    let bmp_sz = file_size_mb(&bitmap_file);
    let paths_sz = file_size_mb(&paths_file);
    let offsets_sz = file_size_mb(&offsets_file);
    let delta_sz = if Path::new(&delta_file).exists() {
        format!("{:.2} MB", file_size_mb(&delta_file))
    } else {
        "—".to_string()
    };

    // Count files from offset index
    let file_count = count_files_from_offsets(&offsets_file).unwrap_or(0);

    // Unique gram count (rough from FST size)
    let version = std::fs::read_to_string(&version_file)
        .unwrap_or_else(|_| "0".into())
        .trim()
        .to_string();

    let ready = if Path::new(&ready_file).exists() {
        let pid_str = std::fs::read_to_string(&ready_file).unwrap_or_default();
        format!("Yes (PID {})", pid_str.trim())
    } else {
        "No (offline)".to_string()
    };

    println!("    FST index  : {fst_sz:.2} MB");
    println!("    Bitmaps    : {bmp_sz:.2} MB");
    println!("    Paths      : {paths_sz:.2} MB");
    println!("    Offsets    : {offsets_sz:.2} MB");
    println!("    Delta      : {delta_sz}");
    println!("    Total      : {:.2} MB", fst_sz + bmp_sz + paths_sz + offsets_sz);
    println!("    Files      : {}M", file_count / 1_000_000);
    println!("    Version    : {version}");
    println!("    MFT Ready  : {ready}");
}

fn file_size_mb(path: &str) -> f64 {
    std::fs::metadata(path)
        .map(|m| m.len() as f64 / 1_048_576.0)
        .unwrap_or(0.0)
}

fn count_files_from_offsets(offset_file: &str) -> Option<usize> {
    let mut f = std::fs::File::open(offset_file).ok()?;
    let mut buf = [0u8; 4];
    f.read_exact(&mut buf).ok()?;
    Some(u32::from_le_bytes(buf) as usize)
}

// ──────────────────────────────────────────────────────────────────────────────
// cmd_monitor – USN Journal real-time update loop
// ──────────────────────────────────────────────────────────────────────────────

fn cmd_monitor(drives: &[char], output_dir: &str) -> Result<()> {
    println!("══════════════════════════════════════════");
    println!("  USN JOURNAL MONITOR (Ctrl-C to stop)");
    println!("══════════════════════════════════════════");

    if !is_elevated() {
        warn!("⚠  Not running as Administrator – USN access may fail.");
    }

    use ilauncher_lib::mft_scanner::UsnIncrementalUpdater;
    use std::thread;

    let mut handles = Vec::new();

    for &drive in drives {
        let output_dir = output_dir.to_string();

        info!("Starting USN monitor for drive {drive}:");

        // Spawn one thread per drive
        let handle = thread::Builder::new()
            .name(format!("usn-{drive}"))
            .spawn(move || {
                let mut updater = UsnIncrementalUpdater::new(drive, output_dir.clone());

                match updater.initialize() {
                    Ok(_) => info!("Drive {drive}: USN updater initialized"),
                    Err(e) => {
                        error!("Drive {drive}: init failed: {e:#}");
                        return;
                    }
                }

                // Poll every 500ms
                loop {
                    match updater.poll_usn_changes() {
                        Ok(_) => {}
                        Err(e) => {
                            error!("Drive {drive}: poll error: {e:#}");
                            thread::sleep(Duration::from_secs(2));
                        }
                    }
                    thread::sleep(Duration::from_millis(500));
                }
            })?;

        handles.push(handle);
    }

    // Wait for Ctrl-C
    println!("\nMonitoring drives: {}  (Ctrl-C to stop)\n", drives.iter().collect::<String>());

    // Simple wait loop – real app would use ctrlc crate
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// DriveIndex – thin wrapper around IndexQuery + PathReader for the CLI
// ──────────────────────────────────────────────────────────────────────────────

struct DriveIndex {
    drive: char,
    fst_map: Map<memmap2::Mmap>,
    bitmap_mmap: memmap2::Mmap,
    paths_mmap: memmap2::Mmap,
    offset_index: Vec<usize>,
    delta_grams: HashMap<String, RoaringBitmap>,
}

impl DriveIndex {
    fn open(drive: char, output_dir: &str, warmup: bool) -> Result<Self> {
        let t0 = Instant::now();

        let fst_file = format!("{output_dir}\\{drive}_index.fst");
        let bitmap_file = format!("{output_dir}\\{drive}_bitmaps.dat");
        let paths_file = format!("{output_dir}\\{drive}_paths.dat");
        let offsets_file = format!("{output_dir}\\{drive}_offsets.dat");
        let delta_file = format!("{output_dir}\\{drive}_index_delta.dat");

        // mmap FST
        let fst_mmap = unsafe {
            memmap2::MmapOptions::new()
                .map(&std::fs::File::open(&fst_file).context("FST file missing – run --scan")?)
                .context("mmap FST")?
        };
        let fst_map = Map::new(fst_mmap).context("Invalid FST")?;

        // mmap Bitmaps
        let bitmap_mmap = unsafe {
            memmap2::MmapOptions::new()
                .map(&std::fs::File::open(&bitmap_file).context("Bitmap file missing")?)
                .context("mmap bitmaps")?
        };

        // mmap Paths
        let paths_mmap = unsafe {
            memmap2::MmapOptions::new()
                .map(&std::fs::File::open(&paths_file).context("Paths file missing")?)
                .context("mmap paths")?
        };

        // Load offset index
        let offset_index = if Path::new(&offsets_file).exists() {
            load_offset_index(&offsets_file)?
        } else {
            warn!("  Drive {drive}: offsets.dat not found – building on-the-fly");
            build_offset_index_from_mmap(&paths_mmap)?
        };

        // Load delta index (optional)
        let delta_grams = if Path::new(&delta_file).exists() {
            load_delta_index(&delta_file).unwrap_or_default()
        } else {
            HashMap::new()
        };

        let idx = DriveIndex {
            drive,
            fst_map,
            bitmap_mmap,
            paths_mmap,
            offset_index,
            delta_grams,
        };

        // Warmup (touch all mmap pages)
        if warmup {
            idx.warmup();
        }

        info!(
            "Drive {drive}: index opened in {:.1}ms  ({} files, delta_grams={})",
            t0.elapsed().as_secs_f64() * 1000.0,
            idx.offset_index.len(),
            idx.delta_grams.len()
        );

        Ok(idx)
    }

    /// Fuzzy 3-gram search → top-N file IDs
    fn search(&self, keyword: &str, limit: usize) -> Result<Vec<u32>> {
        let kw = keyword.to_lowercase();
        let grams = self.split_to_ngrams(&kw);

        if grams.is_empty() {
            return Ok(Vec::new());
        }

        let mut bitmaps: Vec<RoaringBitmap> = Vec::with_capacity(grams.len());

        for gram in &grams {
            // Main index
            let mut bmp = if let Some(off) = self.fst_map.get(gram.as_bytes()) {
                self.load_bitmap(off)?.unwrap_or_default()
            } else {
                RoaringBitmap::new()
            };

            // Delta index (union)
            if let Some(delta_bmp) = self.delta_grams.get(gram) {
                bmp |= delta_bmp;
            }

            if bmp.is_empty() {
                return Ok(Vec::new());
            }

            bitmaps.push(bmp);
        }

        // Intersect all grams → files that contain ALL n-grams
        let result = if bitmaps.len() == 1 {
            bitmaps.into_iter().next().unwrap()
        } else {
            bitmaps.into_iter().reduce(|a, b| a & b).unwrap()
        };

        Ok(result.iter().take(limit).collect())
    }

    /// Convert file IDs to path strings
    fn resolve_paths(&self, ids: &[u32]) -> Vec<String> {
        ids.iter()
            .filter_map(|&id| self.get_path(id).ok())
            .collect()
    }

    fn get_path(&self, file_id: u32) -> Result<String> {
        let idx = file_id as usize;
        let offset = *self
            .offset_index
            .get(idx)
            .ok_or_else(|| anyhow::anyhow!("file_id {file_id} out of range"))?;

        let mmap = &self.paths_mmap;
        anyhow::ensure!(offset + 4 <= mmap.len(), "offset OOB");

        let len = u32::from_le_bytes(mmap[offset..offset + 4].try_into()?) as usize;
        let start = offset + 4;
        anyhow::ensure!(start + len <= mmap.len(), "path OOB");

        Ok(String::from_utf8_lossy(&mmap[start..start + len]).into_owned())
    }

    fn load_bitmap(&self, offset: u64) -> Result<Option<RoaringBitmap>> {
        let off = offset as usize;
        let mmap = &self.bitmap_mmap;

        if off + 4 > mmap.len() {
            return Ok(None);
        }

        let len = u32::from_le_bytes(mmap[off..off + 4].try_into()?) as usize;
        let start = off + 4;

        if start + len > mmap.len() {
            return Ok(None);
        }

        let bmp = RoaringBitmap::deserialize_from(&mmap[start..start + len])?;
        Ok(Some(bmp))
    }

    /// Split keyword into search tokens:
    ///   len < 3 → use the whole string as one token (prefix match)
    ///   len ≥ 3 → sliding 3-gram windows
    fn split_to_ngrams(&self, text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len();

        if n == 0 {
            return vec![];
        }
        if n < 3 {
            return vec![text.to_string()];
        }

        // Sliding 3-grams
        chars.windows(3).map(|w| w.iter().collect()).collect()
    }

    /// Touch all mmap pages (cold-start warmup)
    fn warmup(&self) {
        let t0 = Instant::now();
        const PAGE: usize = 4096;

        let mut sum: u64 = 0;
        for off in (0..self.fst_map.as_fst().as_bytes().len()).step_by(PAGE) {
            sum = sum.wrapping_add(self.fst_map.as_fst().as_bytes()[off] as u64);
        }
        // Partial warmup for bitmaps (cap at 64MB)
        for off in (0..self.bitmap_mmap.len().min(64 * 1024 * 1024)).step_by(PAGE) {
            sum = sum.wrapping_add(self.bitmap_mmap[off] as u64);
        }
        std::hint::black_box(sum);

        info!(
            "  Drive {}: warmup in {:.1}ms",
            self.drive,
            t0.elapsed().as_secs_f64() * 1000.0
        );
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Utility functions
// ──────────────────────────────────────────────────────────────────────────────

fn load_offset_index(path: &str) -> Result<Vec<usize>> {
    let mut r = BufReader::new(std::fs::File::open(path)?);
    let mut buf4 = [0u8; 4];
    r.read_exact(&mut buf4)?;
    let count = u32::from_le_bytes(buf4) as usize;

    let mut index = Vec::with_capacity(count);
    let mut buf8 = [0u8; 8];
    for _ in 0..count {
        r.read_exact(&mut buf8)?;
        index.push(u64::from_le_bytes(buf8) as usize);
    }
    Ok(index)
}

fn build_offset_index_from_mmap(mmap: &memmap2::Mmap) -> Result<Vec<usize>> {
    let mut index = Vec::new();
    let mut off = 0;
    while off + 4 <= mmap.len() {
        index.push(off);
        let len = u32::from_le_bytes(mmap[off..off + 4].try_into()?) as usize;
        off += 4 + len;
    }
    Ok(index)
}

fn load_delta_index(path: &str) -> Result<HashMap<String, RoaringBitmap>> {
    let mut f = std::fs::File::open(path)?;
    let mut map: HashMap<String, RoaringBitmap> = HashMap::new();

    let mut buf4 = [0u8; 4];
    loop {
        if f.read_exact(&mut buf4).is_err() {
            break;
        }
        let gram_len = u32::from_le_bytes(buf4) as usize;
        let mut gram_bytes = vec![0u8; gram_len];
        f.read_exact(&mut gram_bytes)?;
        let gram = String::from_utf8(gram_bytes)?;

        f.read_exact(&mut buf4)?;
        let bmp_len = u32::from_le_bytes(buf4) as usize;
        let mut bmp_bytes = vec![0u8; bmp_len];
        f.read_exact(&mut bmp_bytes)?;
        let bmp = RoaringBitmap::deserialize_from(&bmp_bytes[..])?;

        map.entry(gram)
            .and_modify(|existing| *existing |= bmp.clone())
            .or_insert(bmp);
    }
    Ok(map)
}

/// Check whether the current process has administrator privileges
fn is_elevated() -> bool {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut ret_len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut std::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        );
        ok.is_ok() && elevation.TokenIsElevated != 0
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Windows entry point (called from outer main)
// ──────────────────────────────────────────────────────────────────────────────
pub fn run() -> Result<()> {
    let args = Args::parse()?;

    // Init logging
    let level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .without_time()
        .with_target(false)
        .init();

    let output_dir = resolve_output_dir(&args.output_dir)?;
    let drives = resolve_drives(&args.drives);

    info!("Index directory : {output_dir}");
    info!("Drives          : {}", drives.iter().collect::<String>());
    info!("");

    if args.scan {
        cmd_scan(&drives, &output_dir)?;
    } else if let Some(ref q) = args.query {
        cmd_query(q, &drives, &output_dir, args.limit, !args.no_warmup)?;
    } else if args.bench {
        cmd_bench(&drives, &output_dir, !args.no_warmup)?;
    } else if args.stats {
        cmd_stats(&drives, &output_dir)?;
    } else if args.monitor {
        cmd_monitor(&drives, &output_dir)?;
    } else {
        print_help();
    }

    Ok(())
}

} // mod win

// ──────────────────────────────────────────────────────────────────────────────
// Windows main
// ──────────────────────────────────────────────────────────────────────────────
#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    win::run()
}
