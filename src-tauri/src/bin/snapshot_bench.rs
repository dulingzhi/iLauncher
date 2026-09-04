// LiveIndex 只读搜索延迟基准（无需管理员、不改快照）
//
// 用法：snapshot_bench <snapshot_path>
// 输出：打开耗时 + 各查询 avg/p95 延迟（JSON + 人类可读）

use std::time::Instant;

use ilauncher_lib::index_v2::LiveIndex;

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("用法: snapshot_bench <snapshot_path>");
            std::process::exit(2);
        }
    };

    let t_open = Instant::now();
    let idx = match LiveIndex::open(std::path::Path::new(&path)) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("❌ 打开快照失败: {:#}", e);
            std::process::exit(1);
        }
    };
    let open_ms = t_open.elapsed().as_secs_f64() * 1000.0;
    let rows = idx.snapshot().row_count();
    println!("✓ 快照打开：{} 行，耗时 {:.1} ms", rows, open_ms);

    // 模拟启动器真实查询：短词 / 中文 / 单字符 / 扩展名 / 长模糊序列
    let queries = [
        "report",
        "notes",
        "简历",
        "配置",
        "t",
        "pdf",
        "steam",
        "2024年度总结报告",
    ];

    let mut results = Vec::new();
    for q in queries {
        // 预热 3 次
        for _ in 0..3 {
            let _ = idx.search(q, 50).unwrap();
        }
        let mut lat = Vec::with_capacity(20);
        let mut hit_count = 0usize;
        for _ in 0..20 {
            let t = Instant::now();
            let hits = idx.search(q, 50).unwrap();
            lat.push(t.elapsed().as_secs_f64() * 1000.0);
            hit_count = hits.len();
        }
        lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let avg = lat.iter().sum::<f64>() / lat.len() as f64;
        let p95 = lat[(lat.len() as f64 * 0.95) as usize];
        println!("  {:<12} hits={:<3} avg={:>7.3} ms  p95={:>7.3} ms", q, hit_count, avg, p95);
        results.push((q, hit_count, avg, p95));
    }

    let json = serde_json::json!({
        "snapshot": path,
        "rows": rows,
        "open_ms": open_ms,
        "queries": results.iter().map(|(q, h, avg, p95)| serde_json::json!({
            "query": q, "hits": h, "avg_ms": avg, "p95_ms": p95
        })).collect::<Vec<_>>(),
    });
    println!("BENCH_RESULT {}", serde_json::to_string_pretty(&json).unwrap());
}
