use rusqlite::{Connection, params};

fn main() {
    let conn = Connection::open_in_memory().unwrap();
    
    conn.execute("
        CREATE VIRTUAL TABLE test_fts USING fts5(name, tokenize = 'ascii')
    ", []).unwrap();
    
    let test_files = vec![
        "opera.exe",
        "_ope",
        "parse_oper.h",
        "chrome.exe",
        "operasoftware",
    ];
    
    for file in &test_files {
        conn.execute("INSERT INTO test_fts (name) VALUES (?)", params![file]).unwrap();
    }
    
    println!("测试 FTS5 前缀匹配语法\n");
    
    let test_queries = vec![
        ("\"opera.exe\" OR \"opera.exe*\"", "当前方案"),
        ("^\"opera.exe\" OR ^\"opera.exe*\"", "caret 前缀（不支持）"),
        ("opera.exe*", "简单前缀"),
    ];
    
    for (query, desc) in test_queries {
        println!("🔍 查询: {} ({})", query, desc);
        
        match conn.prepare("SELECT name FROM test_fts WHERE name MATCH ?") {
            Ok(mut stmt) => {
                match stmt.query_map(params![query], |row| row.get::<_, String>(0)) {
                    Ok(rows) => {
                        let results: Vec<String> = rows.filter_map(|r| r.ok()).collect();
                        if results.is_empty() {
                            println!("   ❌ 无结果");
                        } else {
                            println!("   ✅ 找到 {} 个结果:", results.len());
                            for r in results {
                                println!("      - {}", r);
                            }
                        }
                    }
                    Err(e) => println!("   ❌ 执行错误: {}", e),
                }
            }
            Err(e) => println!("   ❌ 准备错误: {}", e),
        }
        println!();
    }
}
