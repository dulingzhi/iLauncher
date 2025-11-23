// 插件安装和管理系统
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use zip::ZipArchive;

/// 插件清单
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
    pub sandbox: SandboxConfig,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
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

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            level: default_sandbox_level(),
            timeout_ms: default_timeout(),
            max_memory_mb: default_memory(),
        }
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

/// 已安装插件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub install_path: PathBuf,
    pub installed_at: chrono::DateTime<chrono::Utc>,
    pub enabled: bool,
    #[serde(default)]
    pub settings: HashMap<String, serde_json::Value>,
}

/// 插件注册表
pub struct PluginRegistry {
    plugins: Arc<RwLock<HashMap<String, InstalledPlugin>>>,
    plugins_dir: PathBuf,
}

impl PluginRegistry {
    pub fn new(plugins_dir: PathBuf) -> Self {
        // 确保插件目录存在
        fs::create_dir_all(&plugins_dir).ok();
        
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            plugins_dir,
        }
    }
    
    /// 加载已安装的插件
    pub async fn load_installed_plugins(&self) -> Result<()> {
        let entries = fs::read_dir(&self.plugins_dir)?;
        let mut plugins = self.plugins.write().await;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // 读取插件 manifest
                let manifest_path = path.join("manifest.json");
                if manifest_path.exists() {
                    let manifest_str = fs::read_to_string(&manifest_path)?;
                    let manifest: PluginManifest = serde_json::from_str(&manifest_str)?;
                    
                    // 读取安装信息
                    let info_path = path.join(".install_info.json");
                    let installed_plugin = if info_path.exists() {
                        let info_str = fs::read_to_string(&info_path)?;
                        serde_json::from_str(&info_str)?
                    } else {
                        InstalledPlugin {
                            manifest: manifest.clone(),
                            install_path: path.clone(),
                            installed_at: chrono::Utc::now(),
                            enabled: true,
                            settings: HashMap::new(),
                        }
                    };
                    
                    plugins.insert(manifest.id.clone(), installed_plugin);
                }
            }
        }
        
        Ok(())
    }
    
    /// 获取已安装插件列表
    pub async fn list_plugins(&self) -> Vec<InstalledPlugin> {
        self.plugins.read().await.values().cloned().collect()
    }
    
    /// 获取插件信息
    pub async fn get_plugin(&self, plugin_id: &str) -> Option<InstalledPlugin> {
        self.plugins.read().await.get(plugin_id).cloned()
    }
    
    /// 检查插件是否已安装
    pub async fn is_installed(&self, plugin_id: &str) -> bool {
        self.plugins.read().await.contains_key(plugin_id)
    }
    
    /// 启用/禁用插件
    pub async fn set_enabled(&self, plugin_id: &str, enabled: bool) -> Result<()> {
        let mut plugins = self.plugins.write().await;
        if let Some(plugin) = plugins.get_mut(plugin_id) {
            plugin.enabled = enabled;
            self.save_plugin_info(plugin)?;
            Ok(())
        } else {
            Err(anyhow!("Plugin not found: {}", plugin_id))
        }
    }
    
    /// 更新插件设置
    pub async fn update_settings(&self, plugin_id: &str, settings: HashMap<String, serde_json::Value>) -> Result<()> {
        let mut plugins = self.plugins.write().await;
        if let Some(plugin) = plugins.get_mut(plugin_id) {
            plugin.settings = settings;
            self.save_plugin_info(plugin)?;
            Ok(())
        } else {
            Err(anyhow!("Plugin not found: {}", plugin_id))
        }
    }
    
    /// 添加已安装插件
    async fn add_plugin(&self, plugin: InstalledPlugin) -> Result<()> {
        let plugin_id = plugin.manifest.id.clone();
        self.plugins.write().await.insert(plugin_id, plugin);
        Ok(())
    }
    
    /// 移除插件
    async fn remove_plugin(&self, plugin_id: &str) -> Result<()> {
        self.plugins.write().await.remove(plugin_id);
        Ok(())
    }
    
    /// 保存插件安装信息
    fn save_plugin_info(&self, plugin: &InstalledPlugin) -> Result<()> {
        let info_path = plugin.install_path.join(".install_info.json");
        let info_json = serde_json::to_string_pretty(plugin)?;
        fs::write(info_path, info_json)?;
        Ok(())
    }
}

/// 插件安装器
pub struct PluginInstaller {
    registry: Arc<PluginRegistry>,
}

impl PluginInstaller {
    pub fn new(registry: Arc<PluginRegistry>) -> Self {
        Self { registry }
    }
    
    /// 安装插件
    pub async fn install(&self, ilp_path: &Path) -> Result<InstalledPlugin> {
        // 1. 验证 .ilp 文件存在
        if !ilp_path.exists() {
            return Err(anyhow!("Plugin file not found: {:?}", ilp_path));
        }
        
        // 2. 打开 ZIP 文件
        let file = fs::File::open(ilp_path)?;
        let mut archive = ZipArchive::new(file)?;
        
        // 3. 读取并解析 manifest.json
        let manifest: PluginManifest = {
            let mut manifest_file = archive.by_name("manifest.json")
                .map_err(|_| anyhow!("manifest.json not found in plugin package"))?;
            
            let mut manifest_str = String::new();
            std::io::Read::read_to_string(&mut manifest_file, &mut manifest_str)?;
            serde_json::from_str(&manifest_str)?
            // manifest_file 在这里 drop
        };
        
        // 4. 验证插件 ID 格式
        if !Self::validate_plugin_id(&manifest.id) {
            return Err(anyhow!("Invalid plugin ID format: {}", manifest.id));
        }
        
        // 5. 检查是否已安装
        if self.registry.is_installed(&manifest.id).await {
            return Err(anyhow!("Plugin already installed: {}", manifest.id));
        }
        
        // 6. 验证签名（生产环境）
        #[cfg(not(debug_assertions))]
        self.verify_signature(&mut archive)?;
        
        // 7. 检查依赖
        self.check_dependencies(&manifest).await?;
        
        // 8. 验证权限
        self.validate_permissions(&manifest)?;
        
        // 9. 解压插件到目标目录
        let install_path = self.registry.plugins_dir.join(&manifest.id);
        fs::create_dir_all(&install_path)?;
        
        Self::extract_archive(&mut archive, &install_path)?;
        
        // 10. 创建安装信息
        let installed_plugin = InstalledPlugin {
            manifest: manifest.clone(),
            install_path: install_path.clone(),
            installed_at: chrono::Utc::now(),
            enabled: true,
            settings: HashMap::new(),
        };
        
        // 11. 保存安装信息
        self.registry.save_plugin_info(&installed_plugin)?;
        
        // 12. 注册到插件注册表
        self.registry.add_plugin(installed_plugin.clone()).await?;
        
        Ok(installed_plugin)
    }
    
    /// 卸载插件
    pub async fn uninstall(&self, plugin_id: &str) -> Result<()> {
        // 1. 获取插件信息
        let plugin = self.registry.get_plugin(plugin_id).await
            .ok_or_else(|| anyhow!("Plugin not found: {}", plugin_id))?;
        
        // 2. 删除插件目录
        if plugin.install_path.exists() {
            fs::remove_dir_all(&plugin.install_path)?;
        }
        
        // 3. 从注册表移除
        self.registry.remove_plugin(plugin_id).await?;
        
        Ok(())
    }
    
    /// 更新插件
    pub async fn update(&self, plugin_id: &str, ilp_path: &Path) -> Result<InstalledPlugin> {
        // 1. 卸载旧版本
        self.uninstall(plugin_id).await?;
        
        // 2. 安装新版本
        self.install(ilp_path).await
    }
    
    /// 验证插件 ID 格式
    fn validate_plugin_id(id: &str) -> bool {
        // 格式: com.author.plugin-name
        let parts: Vec<&str> = id.split('.').collect();
        parts.len() >= 3 && parts.iter().all(|p| !p.is_empty())
    }
    
    /// 验证签名
    #[cfg(not(debug_assertions))]
    fn verify_signature(&self, archive: &mut ZipArchive<fs::File>) -> Result<()> {
        // TODO: 实现 RSA 签名验证
        // 1. 读取 signature.sig
        // 2. 提取除签名外的所有文件
        // 3. 计算 SHA-256 哈希
        // 4. 使用公钥验证签名
        
        // 当前跳过验证（开发阶段）
        Ok(())
    }
    
    /// 检查依赖
    async fn check_dependencies(&self, manifest: &PluginManifest) -> Result<()> {
        for dep in &manifest.dependencies {
            // 检查依赖是否已安装
            let installed = self.registry.get_plugin(&dep.id).await;
            if installed.is_none() {
                return Err(anyhow!("Missing dependency: {} ({})", dep.id, dep.version));
            }
            
            // TODO: 验证版本兼容性（semver）
            // 当前跳过版本检查
        }
        Ok(())
    }
    
    /// 验证权限
    fn validate_permissions(&self, manifest: &PluginManifest) -> Result<()> {
        // 验证权限格式
        for permission in &manifest.permissions {
            if !Self::is_valid_permission(permission) {
                return Err(anyhow!("Invalid permission: {}", permission));
            }
        }
        Ok(())
    }
    
    /// 验证权限格式
    fn is_valid_permission(permission: &str) -> bool {
        let valid_prefixes = [
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
        
        valid_prefixes.iter().any(|prefix| permission.starts_with(prefix))
    }
    
    /// 解压 ZIP 到目标目录
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_plugin_id() {
        assert!(PluginInstaller::validate_plugin_id("com.example.my-plugin"));
        assert!(PluginInstaller::validate_plugin_id("com.github.user.plugin"));
        assert!(!PluginInstaller::validate_plugin_id("invalid"));
        assert!(!PluginInstaller::validate_plugin_id("com."));
        assert!(!PluginInstaller::validate_plugin_id(""));
    }
    
    #[test]
    fn test_is_valid_permission() {
        assert!(PluginInstaller::is_valid_permission("network:api.example.com"));
        assert!(PluginInstaller::is_valid_permission("filesystem:read:~/Documents"));
        assert!(PluginInstaller::is_valid_permission("clipboard:read"));
        assert!(!PluginInstaller::is_valid_permission("invalid:permission"));
    }
}
