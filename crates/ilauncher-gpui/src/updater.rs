//! 更新检查与安装：对接 GitHub releases latest.json 协议（沿用旧版 Tauri 的
//! 静态 JSON 格式；发布侧见 scripts/pack-gpui.ps1 + .github/workflows/release.yml）。
//!
//! 分层：manifest 解析 / 版本比较 / 状态机文案是纯逻辑（全单测，无 I/O）；
//! 网络下载与签名校验是薄 I/O 层（gpui-kit 自带 ReqwestClient，rustls）。
//!
//! 协议样例（latest.json）：
//! ```json
//! {
//!   "version": "1.2.3",
//!   "notes": "…",
//!   "pub_date": "2026-01-01T00:00:00Z",
//!   "platforms": {
//!     "windows-x86_64": { "signature": "<base64 minisign>", "url": "<setup.exe>" }
//!   }
//! }
//! ```

use std::collections::HashMap;

use anyhow::{Context as _, Result, anyhow, bail};
use gpui_kit::http_client::{AsyncBody, HttpClient};
use serde::Deserialize;

/// GitHub releases 的 latest.json 地址（发布通道现行值）
pub const DEFAULT_MANIFEST_URL: &str =
    "https://github.com/dulingzhi/iLauncher/releases/latest/download/latest.json";
/// minisign 公钥（base64，嵌入安装包；配对的私钥由发布方保管，签名见 pack-gpui.ps1）
pub const UPDATE_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDFEQ0Y1MDQ1RjE2OTU0ODQKUldTRVZHbnhSVkRQSFRMcFNRei9FSlFZS2dMN1JZbG5URlA3RG9XVWtDem1McVNoRXVxOXczbkYK";
/// latest.json platforms 下 Windows x64 对应的 key
pub const WINDOWS_PLATFORM: &str = "windows-x86_64";
/// 当前应用版本（开发构建为 0.1.0，检测到正式版会提示）
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize)]
struct Manifest {
    version: String,
    #[serde(default)]
    notes: String,
    #[serde(default, rename = "pub_date")]
    pub_date: Option<String>,
    platforms: HashMap<String, Platform>,
}

#[derive(Deserialize)]
struct Platform {
    signature: String,
    url: String,
}

/// 解析后的更新信息（platforms.windows-x86_64 + 顶层元数据）
#[derive(Clone, Debug, PartialEq)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub signature: String,
    pub pub_date: Option<String>,
}

/// 解析 latest.json，取 Windows x64 平台块；缺平台/字段时报错
pub fn parse_manifest(json: &str) -> Result<UpdateInfo> {
    let m: Manifest = serde_json::from_str(json).context("latest.json 格式非法")?;
    let p = m
        .platforms
        .get(WINDOWS_PLATFORM)
        .with_context(|| format!("latest.json 缺少 {WINDOWS_PLATFORM} 平台块"))?;
    Ok(UpdateInfo {
        version: m.version,
        notes: m.notes,
        url: p.url.clone(),
        signature: p.signature.clone(),
        pub_date: m.pub_date,
    })
}

/// manifest 地址：环境变量 ILAUNCHER_UPDATE_URL 可覆盖（内网/测试源），否则用默认
pub fn manifest_url() -> String {
    manifest_url_from(std::env::var("ILAUNCHER_UPDATE_URL").ok())
}

fn manifest_url_from(env_override: Option<String>) -> String {
    match env_override {
        Some(u) if !u.trim().is_empty() => u,
        _ => DEFAULT_MANIFEST_URL.to_string(),
    }
}

/// 是否需要更新：语义化三段版本比较（容忍 "v" 前缀与 +build 后缀；
/// 同号时预发布版 < 正式版；latest 无法解析时保守返回 false）
pub fn needs_update(current: &str, latest: &str) -> bool {
    match (version_key(current), version_key(latest)) {
        (Some(c), Some(l)) => c < l,
        _ => false,
    }
}

/// "v1.2.3" / "1.2.3+build" / "1.2.3-beta" → (1, 2, 3, release?)
/// 第 4 位：有预发布后缀记 0（更低），正式版/仅构建后缀记 1。
/// 预发布之间不比较先后（beta vs rc 视为同级），对更新判断足够。
fn version_key(v: &str) -> Option<(u64, u64, u64, u8)> {
    let v = v.trim().trim_start_matches('v');
    // 按先出现的分隔符切：'-' 开头 = 预发布（更低），'+' 开头 = 仅构建后缀
    let (core, prerelease) = match v.split_once(['-', '+']) {
        Some((c, _)) if v[c.len()..].starts_with('-') => (c, 0u8),
        Some((c, _)) => (c, 1u8),
        None => (v, 1u8),
    };
    let mut it = core.split('.');
    let parse = |s: &str| s.parse::<u64>().ok();
    let a = parse(it.next()?)?;
    let b = parse(it.next().unwrap_or("0"))?;
    let c = parse(it.next().unwrap_or("0"))?;
    Some((a, b, c, prerelease))
}

/// 更新流程状态机（设置页「检查更新」按钮的展示与可点状态都从这里出）
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    /// 尚未检查 / 检查失败后回到可重试
    Idle,
    Checking,
    UpToDate(String),
    Available(UpdateInfo),
    Downloading(String),
    /// 已下载且签名校验通过，等待用户确认安装
    Ready { version: String, path: std::path::PathBuf },
    Failed(String),
}

impl UpdateState {
    pub fn button_label(&self) -> String {
        use crate::i18n::t;
        match self {
            UpdateState::Idle => t!("updater.button_idle").into(),
            UpdateState::Checking => t!("updater.button_checking").into(),
            UpdateState::UpToDate(v) => t!("updater.button_uptodate", version = v.as_str()).into(),
            UpdateState::Available(info) => {
                t!("updater.button_available", version = info.version.as_str()).into()
            }
            UpdateState::Downloading(v) => {
                t!("updater.button_downloading", version = v.as_str()).into()
            }
            UpdateState::Ready { version, .. } => {
                t!("updater.button_ready", version = version.as_str()).into()
            }
            UpdateState::Failed(_) => t!("updater.button_failed").into(),
        }
    }

    /// 按钮当前是否可点击（进行中/已最新/已就绪但即将退出进程的状态除外）
    pub fn can_click(&self) -> bool {
        matches!(
            self,
            UpdateState::Idle | UpdateState::Failed(_) | UpdateState::Available(_) | UpdateState::Ready { .. }
        )
    }

    /// 设置项描述行（构建页时取当前状态）
    pub fn status_text(&self) -> String {
        use crate::i18n::t;
        match self {
            UpdateState::Idle => t!("updater.status_idle").into(),
            UpdateState::Checking => t!("updater.status_checking").into(),
            UpdateState::UpToDate(_) => t!("updater.status_uptodate").into(),
            UpdateState::Available(info) => t!(
                "updater.status_available",
                version = info.version.as_str(),
                date = info.pub_date.as_deref().unwrap_or(t!("updater.date_unknown").as_ref())
            )
            .into(),
            UpdateState::Downloading(v) => {
                t!("updater.status_downloading", version = v.as_str()).into()
            }
            UpdateState::Ready { .. } => t!("updater.status_ready").into(),
            UpdateState::Failed(e) => t!("updater.status_failed", error = e.as_str()).into(),
        }
    }
}

/// 检查更新：拉 latest.json → 解析 → 版本比较。current >= latest 返回 UpToDate。
pub async fn check(client: &dyn HttpClient, current_version: &str) -> Result<UpdateState> {
    let mut resp = client
        .get(&manifest_url(), AsyncBody::empty(), true)
        .await
        .context("拉取 latest.json 失败（网络/代理?）")?;
    let mut body = Vec::new();
    futures::AsyncReadExt::read_to_end(resp.body_mut(), &mut body).await?;
    if !resp.status().is_success() {
        bail!("latest.json 返回 HTTP {}", resp.status().as_u16());
    }
    let info = parse_manifest(&String::from_utf8_lossy(&body))?;
    if needs_update(current_version, &info.version) {
        Ok(UpdateState::Available(info))
    } else {
        Ok(UpdateState::UpToDate(current_version.to_string()))
    }
}

/// 下载安装包到临时目录并做 minisign 签名校验；通过后返回文件路径
pub async fn download(client: &dyn HttpClient, info: &UpdateInfo) -> Result<std::path::PathBuf> {
    let mut resp = client
        .get(&info.url, AsyncBody::empty(), true)
        .await
        .with_context(|| format!("下载 {} 失败", info.url))?;
    let mut bytes = Vec::new();
    futures::AsyncReadExt::read_to_end(resp.body_mut(), &mut bytes).await?;
    if !resp.status().is_success() {
        bail!("安装包返回 HTTP {}", resp.status().as_u16());
    }
    verify_signature(&bytes, &info.signature).context("签名校验失败（文件可能被篡改）")?;

    let dir = std::env::temp_dir();
    let path = dir.join(format!("iLauncher-update-{}.exe", info.version));
    std::fs::write(&path, &bytes).with_context(|| format!("写入 {} 失败", path.display()))?;
    Ok(path)
}

/// minisign 签名校验（公钥嵌入，等价 tauri-plugin-updater 的验签步骤）。
/// 注意两层 base64：内置公钥 / latest.json 的 signature 字段
/// 都是 minisign 文件内容的 base64；minisign-verify 要的是解码后的多行文档
/// （公钥用 PublicKey::decode 取第二行，签名用 Signature::decode 取四行结构）
pub fn verify_signature(data: &[u8], signature_b64: &str) -> Result<()> {
    use base64::Engine as _;
    use minisign_verify::{PublicKey, Signature};

    let public_key = {
        let doc = base64::engine::general_purpose::STANDARD
            .decode(UPDATE_PUBKEY)
            .context("内置公钥 base64 解码失败")?;
        let doc = String::from_utf8(doc).context("内置公钥非 UTF-8")?;
        PublicKey::decode(&doc).context("内置公钥格式非法")?
    };
    let signature = {
        let doc = base64::engine::general_purpose::STANDARD
            .decode(signature_b64.trim())
            .context("latest.json 签名字段 base64 解码失败")?;
        let doc = String::from_utf8(doc).context("签名字段非 UTF-8")?;
        Signature::decode(&doc).context("latest.json 签名字段格式非法")?
    };
    public_key
        .verify(data, &signature, false)
        .map_err(|e| anyhow!("{e}"))
}

/// 启动安装程序（NSIS passive：/SILENT 显示进度、无交互，等价 tauri installMode）。
/// 调用方应在 spawn 后退出本进程，释放 exe 文件锁让安装程序替换。
#[cfg(windows)]
pub fn launch_installer(path: &std::path::Path) -> Result<()> {
    std::process::Command::new(path)
        .arg("/SILENT")
        .spawn()
        .context("启动安装程序失败")?;
    Ok(())
}

#[cfg(not(windows))]
pub fn launch_installer(path: &std::path::Path) -> Result<()> {
    let _ = path;
    bail!("安装程序仅支持 Windows")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "version": "1.2.3",
        "notes": "release notes",
        "pub_date": "2026-01-01T00:00:00Z",
        "platforms": {
            "windows-x86_64": {
                "signature": "sig-b64",
                "url": "https://example.com/iLauncher_1.2.3_x64-setup.exe"
            },
            "linux-x86_64": { "signature": "s", "url": "u" }
        }
    }"#;

    #[test]
    fn parse_manifest_extracts_windows_platform() {
        let info = parse_manifest(SAMPLE).unwrap();
        assert_eq!(info.version, "1.2.3");
        assert_eq!(info.url, "https://example.com/iLauncher_1.2.3_x64-setup.exe");
        assert_eq!(info.signature, "sig-b64");
        assert_eq!(info.pub_date.as_deref(), Some("2026-01-01T00:00:00Z"));
    }

    #[test]
    fn parse_manifest_missing_platform_errors() {
        let json = r#"{"version":"1.0.0","platforms":{"darwin-aarch64":{"signature":"s","url":"u"}}}"#;
        assert!(parse_manifest(json).is_err());
    }

    #[test]
    fn parse_manifest_bad_json_errors() {
        assert!(parse_manifest("not json").is_err());
    }

    #[test]
    fn manifest_url_env_override_and_default() {
        assert_eq!(
            manifest_url_from(Some(" https://mirror.example/latest.json ".to_string())),
            " https://mirror.example/latest.json "
        );
        assert_eq!(manifest_url_from(None), DEFAULT_MANIFEST_URL);
        assert_eq!(manifest_url_from(Some("  ".to_string())), DEFAULT_MANIFEST_URL);
    }

    #[test]
    fn needs_update_matrix() {
        // 同级相等 / 各级更大
        assert!(!needs_update("1.2.3", "1.2.3"));
        assert!(needs_update("1.2.3", "1.2.4"));
        assert!(needs_update("1.2.3", "1.3.0"));
        assert!(needs_update("1.2.3", "2.0.0"));
        assert!(!needs_update("1.2.3", "1.2.2"));
        assert!(!needs_update("2.0.0", "1.9.9"));
        // v 前缀 / 构建后缀
        assert!(needs_update("v1.0.0", "v1.0.1"));
        assert!(!needs_update("1.0.0+build42", "1.0.0"));
        // 预发布 < 同号正式
        assert!(needs_update("1.0.0-beta", "1.0.0"));
        // latest 无法解析 → 保守不更新
        assert!(!needs_update("1.0.0", "garbage"));
        assert!(!needs_update("1.0.0", ""));
    }

    #[test]
    fn state_button_and_clickable_consistency() {
        crate::i18n::test_use_zh();
        assert_eq!(UpdateState::Idle.button_label(), "检查更新");
        assert!(UpdateState::Idle.can_click());
        assert!(!UpdateState::Checking.can_click());
        assert!(UpdateState::Failed("x".into()).can_click());
        assert!(!UpdateState::UpToDate("1.0.0".into()).can_click());
        let info = UpdateInfo {
            version: "1.2.3".into(),
            notes: String::new(),
            url: "u".into(),
            signature: "s".into(),
            pub_date: None,
        };
        assert_eq!(
            UpdateState::Available(info.clone()).button_label(),
            "下载并安装 v1.2.3"
        );
        assert!(UpdateState::Available(info.clone()).can_click());
        assert!(!UpdateState::Downloading("1.2.3".into()).can_click());
        let ready = UpdateState::Ready {
            version: "1.2.3".into(),
            path: std::path::PathBuf::from("x.exe"),
        };
        assert_eq!(ready.button_label(), "安装并重启（v1.2.3）");
        assert!(ready.can_click());
    }

    /// 真实网络冒烟（默认 --ignored）：拉 GitHub releases 的 latest.json 走完整检查链。
    /// 运行：cargo test -- --ignored live_check
    /// 断言口径：不许 Failed（网络可达 + JSON 可解析）；版本结论只打印不硬断言
    #[test]
    #[ignore]
    fn live_check_hits_github_releases() {
        let client =
            reqwest_client::ReqwestClient::user_agent("iLauncher/live-check").unwrap();
        let state = reqwest_client::runtime()
            .block_on(check(&client, "0.0.1"))
            .expect("live check 应返回状态而不是 Err");
        assert!(
            !matches!(state, UpdateState::Failed(_)),
            "live check 不得失败: {}",
            state.status_text()
        );
        println!("LIVE_CHECK_OK → {}", state.button_label());
    }

    /// 真实下载 + minisign 验签冒烟（默认 --ignored）：验证 download() 全链路。
    /// 运行：cargo test -- --ignored live_download
    #[test]
    #[ignore]
    fn live_download_and_verify_signature() {
        let client =
            reqwest_client::ReqwestClient::user_agent("iLauncher/live-check").unwrap();
        let state = reqwest_client::runtime()
            .block_on(check(&client, "0.0.1"))
            .expect("live check");
        let UpdateState::Available(info) = state else {
            panic!("预期有新版本可下载，实际: {}", state.button_label());
        };
        let path = reqwest_client::runtime()
            .block_on(download(&client, &info))
            .expect("下载或验签失败");
        let size = std::fs::metadata(&path).unwrap().len();
        assert!(size > 1_000_000, "安装包异常小: {size}");
        println!("LIVE_DOWNLOAD_OK → {} ({} bytes)", path.display(), size);
        let _ = std::fs::remove_file(&path);
    }
}
