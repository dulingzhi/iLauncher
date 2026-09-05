// 冒烟：启动监听 5 秒，验证 win32 消息窗口创建 + 事件循环可进可退。
// 运行：cargo run --example monitor_smoke
use ilauncher_clipboard::{monitor, ClipboardStore};
use parking_lot::Mutex;
use std::sync::Arc;

fn main() {
    let store = Arc::new(Mutex::new(ClipboardStore::in_memory(100)));
    let handle = monitor::start_monitor(store.clone());

    // 模拟外部写入（等价于 WM_CLIPBOARDUPDATE 后的读取路径）
    std::thread::sleep(std::time::Duration::from_millis(500));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let added = store.lock().add_text("smoke-test-content", now);
    println!("STORE_ADD_OK={added} LEN={}", store.lock().len());

    // 真实事件路径：写系统剪贴板 → WM_CLIPBOARDUPDATE → 监听线程读回 store
    std::thread::sleep(std::time::Duration::from_millis(500));
    if let Ok(mut cb) = arboard::Clipboard::new() {
        let _ = cb.set_text("clipboard-event-driven-内容");
        println!("CLIPBOARD_SET_OK");
    } else {
        println!("CLIPBOARD_SET_SKIP");
    }
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let found = store
        .lock()
        .search("clipboard-event-driven", 10)
        .first()
        .map(|it| it.content.clone());
    println!("EVENT_PATH_RESULT={:?}", found);

    std::thread::sleep(std::time::Duration::from_millis(3000));
    monitor::request_shutdown();
    let _ = handle.join();
    println!("MONITOR_SMOKE_DONE");
}
