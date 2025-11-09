use rusqlite::{Connection, params};

fn main() {
    let conn = Connection::open_in_memory().unwrap();
    
    // 创建 FTS5 测试表
    conn.execute("
        CREATE VIRTUAL TABLE test_fts USING fts5(name, tokenize = 'ascii')
    ", []).unwrap();
    
    // 插入测试数据
    let test_files = vec![
        "opera",
        "opera.exe",
        "opera.exe.bak",
        "operasoftware",
    ];
    
    for file in &test_files {
        conn.execute("INSERT INTO test_fts (name) VALUES (?)", params![file]).unwrap();
    }
    
    println!("测试 FTS5 查询语法\n");
    
    // 测试不同的查询语法
    let test_queries = vec![
        ("opera*", "简单前缀匹配"),
        ("\"opera*\"", "双引号前缀"),
        ("opera.exe*", "点号前缀（无转义）"),
        ("\"opera.exe*\"", "点号前缀（双引号）"),
        ("opera OR opera*", "OR 组合（无引号）"),
        ("\"opera\" OR \"opera*\"", "OR 组合（双引号）"),
        ("^opera.exe", "caret 精确匹配"),
        ("{opera.exe}", "花括号"),
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
