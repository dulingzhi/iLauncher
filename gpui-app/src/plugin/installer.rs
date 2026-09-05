//! 插件安装与已安装状态管理（对齐 src-tauri/src/plugin/plugin_installer.rs）。
//! 无 gpui 依赖，可单元测试（tempdir 构造假 .ilp 走完整安装回路）。
//!
//! 相对 Tauri 版的偏离：
//!   - 同步 fs + parking_lot（Tauri 版 tokio RwLock；本 crate 无 tokio 依赖）
//!   - installed_at 用 Unix 秒 u64（无 chrono 依赖，与 audit.rs 一致）
//!   - 不迁移 verify_signature：Tauri 版是 #[cfg(not(debug_assertions))] 的空 TODO
//!     stub（生产直接 Ok），迁移即搬运死代码；真验签随市场公钥方案一起做
//!   - 不迁移 update() 的独立方法：卸载+安装两步调用方组合即可（无额外逻辑）

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

/// 插件清单（对齐 Tauri PluginManifest 字段集）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: PluginAuthor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<PluginRepository>,
    pub license: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub icon: String,
    pub engine: PluginEngine,
    pub triggers: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub sandbox: ManifestSandboxConfig,
    #[serde(default)]
    pub settings: Vec<PluginSettingDef>,
    #[serde(default)]
    pub dependencies: Vec<PluginDependency>,
    #[serde(default)]
    pub changelog: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAuthor {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRepository {
    pub r#type: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEngine {
    pub r#type: String, // "wasm", "javascript", "native"
    pub entry: String,
    pub runtime_version: String,
}

/// 清单内嵌沙盒配置（名字区别于 sandbox.rs 的运行时 SandboxConfig）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSandboxConfig {
    #[serde(default = "default_sandbox_level")]
    pub level: String, // "none", "basic", "restricted", "strict"
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default = "default_memory")]
    pub max_memory_mb: u64,
}

fn default_sandbox_level() -> String {
    "restricted".to_string()
}

fn default_timeout() -> u64 {
    5000
}

fn default_memory() -> u64 {
    50
}

impl Default for ManifestSandboxConfig {
    fn default() -> Self {
        Self { level: default_sandbox_level(), timeout_ms: default_timeout(), max_memory_mb: default_memory() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSettingDef {
    pub key: String,
    pub r#type: String, // "string", "number", "boolean", "enum"
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    pub id: String,
    pub version: String,
}

/// 已安装插件信息（installed_at：Unix 秒，偏离 Tauri chrono）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub install_path: PathBuf,
    pub installed_at: u64,
    pub enabled: bool,
    #[serde(default)]
    pub settings: HashMap<String, serde_json::Value>,
}

/// 插件注册表：已安装插件的内存索引 + 磁盘状态
pub struct PluginRegistry {
    plugins: RwLock<HashMap<String, InstalledPlugin>>,
    plugins_dir: PathBuf,
}

impl PluginRegistry {
    pub fn new(plugins_dir: PathBuf) -> Self {
        fs::create_dir_all(&plugins_dir).ok();
        Self { plugins: RwLock::new(HashMap::new()), plugins_dir }
    }

    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    /// 扫描插件目录加载已安装插件（manifest.json + .install_info.json）
    pub fn load_installed(&self) -> Result<()> {
        let mut plugins = self.plugins.write();
        plugins.clear();
        for entry in fs::read_dir(&self.plugins_dir)? {
            let path = entry?.path();
            if !path.is_dir() {
                continue;
            }
            let manifest_path = path.join("manifest.json");
            if !manifest_path.exists() {
                continue;
            }
            let manifest: PluginManifest =
                serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
            let installed = match path.join(".install_info.json") {
                info_path if info_path.exists() => {
                    serde_json::from_str(&fs::read_to_string(&info_path)?)?
                }
                _ => InstalledPlugin {
                    manifest: manifest.clone(),
                    install_path: path.clone(),
                    installed_at: now_unix(),
                    enabled: true,
                    settings: HashMap::new(),
                },
            };
            plugins.insert(manifest.id.clone(), installed);
        }
        Ok(())
    }

    pub fn list(&self) -> Vec<InstalledPlugin> {
        self.plugins.read().values().cloned().collect()
    }

    pub fn get(&self, plugin_id: &str) -> Option<InstalledPlugin> {
        self.plugins.read().get(plugin_id).cloned()
    }

    pub fn is_installed(&self, plugin_id: &str) -> bool {
        self.plugins.read().contains_key(plugin_id)
    }

    /// 启用/禁用（持久化到 .install_info.json）
    pub fn set_enabled(&self, plugin_id: &str, enabled: bool) -> Result<()> {
        let mut plugins = self.plugins.write();
        let plugin = plugins.get_mut(plugin_id).ok_or_else(|| anyhow!("Plugin not found: {plugin_id}"))?;
        plugin.enabled = enabled;
        save_install_info(plugin)
    }

    /// 注册表内更新（安装流程用）
    pub(crate) fn insert(&self, plugin: InstalledPlugin) {
        self.plugins.write().insert(plugin.manifest.id.clone(), plugin);
    }

    /// 注册表内移除（卸载流程用）
    pub(crate) fn remove(&self, plugin_id: &str) {
        self.plugins.write().remove(plugin_id);
    }
}

/// 插件安装器：.ilp（zip）安装 / 卸载
pub struct PluginInstaller {
    registry: Arc<PluginRegistry>,
}

impl PluginInstaller {
    pub fn new(registry: Arc<PluginRegistry>) -> Self {
        Self { registry }
    }

    /// 安装 .ilp 包：验证 → 解压 → 登记（对齐 Tauri install 步骤 1-12，
    /// 去掉空验签 stub）
    pub fn install(&self, ilp_path: &Path) -> Result<InstalledPlugin> {
        if !ilp_path.exists() {
            return Err(anyhow!("插件包不存在: {}", ilp_path.display()));
        }

        let file = fs::File::open(ilp_path)?;
        let mut archive = ZipArchive::new(file).map_err(|e| anyhow!("非法 .ilp（zip 损坏）: {e}"))?;

        // manifest.json 必在包内
        let manifest: PluginManifest = {
            let mut mf = archive
                .by_name("manifest.json")
                .map_err(|_| anyhow!("包内缺 manifest.json"))?;
            let mut s = String::new();
            std::io::Read::read_to_string(&mut mf, &mut s)?;
            serde_json::from_str(&s).map_err(|e| anyhow!("manifest.json 解析失败: {e}"))?
        };

        if !Self::validate_plugin_id(&manifest.id) {
            return Err(anyhow!("非法插件 ID 格式: {}", manifest.id));
        }
        if self.registry.is_installed(&manifest.id) {
            return Err(anyhow!("插件已安装: {}", manifest.id));
        }
        Self::check_dependencies(&self.registry, &manifest)?;
        Self::validate_permissions(&manifest)?;

        // 解压到 <plugins_dir>/<id>/
        let install_path = self.registry.plugins_dir().join(&manifest.id);
        fs::create_dir_all(&install_path)?;
        Self::extract_archive(&mut archive, &install_path)?;

        let installed = InstalledPlugin {
            manifest: manifest.clone(),
            install_path: install_path.clone(),
            installed_at: now_unix(),
            enabled: true,
            settings: HashMap::new(),
        };
        save_install_info(&installed)?;
        self.registry.insert(installed.clone());
        Ok(installed)
    }

    /// 卸载：删目录 + 登记表移除
    pub fn uninstall(&self, plugin_id: &str) -> Result<()> {
        let plugin = self
            .registry
            .get(plugin_id)
            .ok_or_else(|| anyhow!("Plugin not found: {plugin_id}"))?;
        if plugin.install_path.exists() {
            fs::remove_dir_all(&plugin.install_path)?;
        }
        self.registry.remove(plugin_id);
        Ok(())
    }

    /// 插件 ID 格式：com.author.plugin-name（≥3 段非空）
    fn validate_plugin_id(id: &str) -> bool {
        let parts: Vec<&str> = id.split('.').collect();
        parts.len() >= 3 && parts.iter().all(|p| !p.is_empty())
    }

    /// 依赖检查：已安装即可（semver 兼容验证 Tauri 同为 TODO，不迁移）
    fn check_dependencies(registry: &PluginRegistry, manifest: &PluginManifest) -> Result<()> {
        for dep in &manifest.dependencies {
            if !registry.is_installed(&dep.id) {
                return Err(anyhow!("缺少依赖: {} ({})", dep.id, dep.version));
            }
        }
        Ok(())
    }

    /// 权限格式白名单（与 Tauri is_valid_permission 逐条一致）
    fn validate_permissions(manifest: &PluginManifest) -> Result<()> {
        const VALID_PREFIXES: [&str; 9] = [
            "network:",
            "filesystem:read:",
            "filesystem:write:",
            "clipboard:read",
            "clipboard:write",
            "system:info",
            "system:execute",
            "database:read",
            "database:write",
        ];
        for permission in &manifest.permissions {
            if !VALID_PREFIXES.iter().any(|prefix| permission.starts_with(prefix)) {
                return Err(anyhow!("非法权限: {permission}"));
            }
        }
        Ok(())
    }

    fn extract_archive(archive: &mut ZipArchive<fs::File>, target_dir: &Path) -> Result<()> {
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = target_dir.join(file.name());
            if file.is_dir() {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut outfile = fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }
        Ok(())
    }
}

/// 保存安装信息（登记 enabling/settings 状态，重启后可恢复）
fn save_install_info(plugin: &InstalledPlugin) -> Result<()> {
    let info_path = plugin.install_path.join(".install_info.json");
    fs::write(&info_path, serde_json::to_string_pretty(plugin)?)?;
    Ok(())
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use zip::write::SimpleFileOptions;

    /// 独立临时目录（测试名派生，避免并行冲突）
    fn tempdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ilauncher_installer_test_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_manifest(id: &str) -> PluginManifest {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "name": "Test Plugin",
            "version": "1.0.0",
            "description": "测试插件",
            "author": { "name": "Tester" },
            "license": "MIT",
            "icon": "🧪",
            "engine": { "type": "javascript", "entry": "main.js", "runtime_version": "1.0" },
            "triggers": [],
            "permissions": ["network:api.example.com"]
        }))
        .unwrap()
    }

    /// 构造假 .ilp 包：manifest.json + 一个代码文件（可选自定义 manifest 值）
    fn make_ilp(dir: &Path, name: &str, manifest: &PluginManifest) -> PathBuf {
        let path = dir.join(name);
        let file = fs::File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let opts = SimpleFileOptions::default();
        writer.start_file("manifest.json", opts).unwrap();
        writer.write_all(serde_json::to_string(manifest).unwrap().as_bytes()).unwrap();
        writer.start_file("main.js", opts).unwrap();
        writer.write_all(b"console.log('hi');").unwrap();
        writer.finish().unwrap();
        path
    }

    fn setup(tag: &str) -> (PathBuf, Arc<PluginRegistry>, PluginInstaller) {
        let dir = tempdir(tag);
        let registry = Arc::new(PluginRegistry::new(dir.join("plugins")));
        let installer = PluginInstaller::new(registry.clone());
        (dir, registry, installer)
    }

    #[test]
    fn install_list_uninstall_roundtrip() {
        let (dir, registry, installer) = setup("roundtrip");
        let manifest = sample_manifest("com.test.hello");
        let ilp = make_ilp(&dir, "hello.ilp", &manifest);

        let installed = installer.install(&ilp).unwrap();
        assert_eq!(installed.manifest.id, "com.test.hello");
        assert!(installed.enabled);
        // 磁盘落盘：manifest 解压 + 安装信息
        assert!(dir.join("plugins\\com.test.hello\\manifest.json").exists());
        assert!(dir.join("plugins\\com.test.hello\\.install_info.json").exists());
        assert!(dir.join("plugins\\com.test.hello\\main.js").exists());
        assert_eq!(registry.list().len(), 1);

        // 重复安装拒绝
        assert!(installer.install(&ilp).is_err());

        installer.uninstall("com.test.hello").unwrap();
        assert!(registry.list().is_empty());
        assert!(!dir.join("plugins\\com.test.hello").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_installed_recovers_state() {
        let (dir, registry, installer) = setup("reload");
        let ilp = make_ilp(&dir, "a.ilp", &sample_manifest("com.test.a"));
        installer.install(&ilp).unwrap();
        registry.set_enabled("com.test.a", false).unwrap();

        // 新注册表从磁盘恢复（模拟重启）
        let registry2 = Arc::new(PluginRegistry::new(dir.join("plugins")));
        registry2.load_installed().unwrap();
        let list = registry2.list();
        assert_eq!(list.len(), 1);
        assert!(!list[0].enabled, "禁用状态应从 .install_info.json 恢复");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_bad_package_and_manifest() {
        let (dir, _, installer) = setup("reject");

        // 非 zip 文件
        let garbage = dir.join("bad.ilp");
        fs::write(&garbage, b"not a zip").unwrap();
        assert!(installer.install(&garbage).is_err());

        // 缺 manifest.json
        let empty = dir.join("empty.ilp");
        let f = fs::File::create(&empty).unwrap();
        zip::ZipWriter::new(f).finish().unwrap();
        assert!(installer.install(&empty).is_err());

        // 非法 ID（两段）
        let bad_id = make_ilp(&dir, "badid.ilp", &sample_manifest("com.short"));
        assert!(installer.install(&bad_id).is_err());

        // 非法权限
        let mut m = sample_manifest("com.test.badperm");
        m.permissions = vec!["camera:use".to_string()];
        let bad_perm = make_ilp(&dir, "badperm.ilp", &m);
        assert!(installer.install(&bad_perm).is_err());

        // 缺依赖
        let mut m = sample_manifest("com.test.needsdep");
        m.dependencies = vec![PluginDependency { id: "com.test.base".into(), version: "1.0".into() }];
        let needs_dep = make_ilp(&dir, "needsdep.ilp", &m);
        assert!(installer.install(&needs_dep).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_plugin_id_rules() {
        assert!(PluginInstaller::validate_plugin_id("com.example.my-plugin"));
        assert!(PluginInstaller::validate_plugin_id("com.github.user.plugin"));
        assert!(!PluginInstaller::validate_plugin_id("invalid"));
        assert!(!PluginInstaller::validate_plugin_id("com."));
        assert!(!PluginInstaller::validate_plugin_id(""));
    }

    #[test]
    fn validate_permission_prefixes() {
        let mut m = sample_manifest("com.test.perm");
        m.permissions = vec![
            "network:api.example.com".into(),
            "filesystem:read:~/Documents".into(),
            "clipboard:read".into(),
            "system:execute".into(),
        ];
        PluginInstaller::validate_permissions(&m).unwrap();

        m.permissions = vec!["invalid:permission".into()];
        assert!(PluginInstaller::validate_permissions(&m).is_err());
    }

    #[test]
    fn manifest_defaults_applied() {
        let m: PluginManifest = serde_json::from_value(serde_json::json!({
            "id": "com.test.min",
            "name": "Min",
            "version": "0.1.0",
            "description": "",
            "author": { "name": "x" },
            "license": "MIT",
            "icon": "",
            "engine": { "type": "wasm", "entry": "x.wasm", "runtime_version": "1" },
            "triggers": []
        }))
        .unwrap();
        // 未提供时走默认值（对齐 Tauri serde default）
        assert_eq!(m.sandbox.level, "restricted");
        assert_eq!(m.sandbox.timeout_ms, 5000);
        assert_eq!(m.sandbox.max_memory_mb, 50);
        assert!(m.permissions.is_empty());
        assert!(m.dependencies.is_empty());
    }
}
