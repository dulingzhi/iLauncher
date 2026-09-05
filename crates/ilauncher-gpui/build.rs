//! 构建脚本：把 assets/app.ico 作为图标资源嵌入 exe。
//!
//! 图标更新流程（与托盘图标同款像素，保持品牌一致）：
//!   1. 修改 tray_icon_gen.rs 里的 SDF 参数
//!   2. `ilauncher-gpui --dump-icon target/icon.rgba`
//!   3. PIL 转多尺寸：`Image.frombytes("RGBA",(128,128),raw).save("assets/app.ico", sizes=[...])`
//!   4. cargo build（本脚本自动重新嵌入）

#[cfg(windows)]
fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app.ico");
    embed_resource::compile("assets/app.rc", embed_resource::NONE);
}

#[cfg(not(windows))]
fn main() {}
