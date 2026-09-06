//! Lua 命令插件：Listary 风格的搜索框命令。
//! 无 gpui 依赖，可单元测试。
//!
//! 两类命令：
//!   - 前缀命令：输入 "hosts 127.0.0.1 dev.local" → 关键字 hosts 触发，其余为参数
//!   - 上下文命令（context = true）：对主列表当前选中的文件操作（如 hash 计算 SHA256），
//!     选中路径经 QueryContext.selection 注入
//!
//! 安全模型：
//!   - 每次调用新建 Lua 实例并剥离 io/os/debug/package/require/load 等全局（脚本无系统入口）
//!   - 系统能力只经 ilauncher.* 宿主 API 暴露，每个 API 内部走 SandboxManager 权限检查
//!     （检查即审计，与 calculator/web_search 同一管道）
//!   - 副作用（open/copy）不直接执行，记入 RunContext 由 execute 返回 ExecuteOutcome，
//!     Launcher 层真正执行——与 trait 头注释"插件保持纯函数"一致
//!   - shell/file 副作用在脚本内直接执行（与 web_search 网络访问同级），全部先过权限检查
//!
//! 脚本协议（可选 preview，必需 run）：
//!   function preview(args, selection) return "标题", "副标题" end
//!   function run(args, selection) return "状态栏反馈文本" end

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context as _, Result, anyhow};
use mlua::{Lua, Value};
use parking_lot::Mutex;

use crate::plugin::sandbox::{PluginPermission, SandboxManager};
use crate::plugin::{ExecuteOutcome, Plugin, PluginAction, PluginMetadata, QueryContext, QueryResult};

/// 命令结果得分：命令置顶的阈值（高于 calculator 的 1000）
pub const COMMAND_SCORE: i32 = 2000;

/// 一条 Lua 命令脚本（内置脚本编译期嵌入；第三方脚本来自 .ilp 安装包的 manifest + entry）
pub struct LuaScript {
    /// 触发关键字（首个为规范名，全部大小写不敏感匹配）
    pub keywords: Vec<String>,
    /// 用法提示（无参/参数不足时的兜底标题）
    pub usage: String,
    /// true = 上下文命令：需要主列表选中文件（QueryContext.selection）
    pub context: bool,
    pub icon: String,
    /// Lua 源码
    pub source: std::borrow::Cow<'static, str>,
}

/// 脚本执行期间的副作用收集（open/copy 上移 Launcher 层执行）
#[derive(Default)]
struct RunContext {
    outcome: Option<ExecuteOutcome>,
}

/// Lua 命令插件（一个实例承载一条命令：权限/禁用/审计按插件 id 归属）
pub struct LuaCommandPlugin {
    metadata: PluginMetadata,
    sandbox: Arc<SandboxManager>,
    script: LuaScript,
    /// 插件私有配置（settings.get/set 宿主 API；hosts_path 等）
    settings: Arc<Mutex<HashMap<String, String>>>,
    /// query 时注入的选中文件路径，execute 时取用（搜索框单窗口，无并发问题）
    pending_selection: Mutex<Option<String>>,
}

impl LuaCommandPlugin {
    pub fn new(
        sandbox: Arc<SandboxManager>,
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        script: LuaScript,
    ) -> Self {
        let keywords = script.keywords.clone();
        let icon = script.icon.clone();
        Self {
            metadata: PluginMetadata::new(id, name)
                .with_description(description)
                .with_icon(icon)
                .with_trigger_keywords(keywords),
            sandbox,
            script,
            settings: Arc::new(Mutex::new(HashMap::new())),
            pending_selection: Mutex::new(None),
        }
    }

    /// 从 .ilp 安装包的 manifest 构造（第三方 Lua 命令插件；沙盒配置由 manager 侧注册）
    pub fn from_manifest(sandbox: Arc<SandboxManager>, manifest: &crate::plugin::installer::PluginManifest, source: String) -> Result<Self> {
        if manifest.engine.r#type != "lua" {
            return Err(anyhow!("非 Lua 引擎插件: {}", manifest.engine.r#type));
        }
        if manifest.triggers.is_empty() {
            return Err(anyhow!("Lua 命令插件至少需要 1 个 trigger 关键字: {}", manifest.id));
        }
        let usage = manifest
            .usage
            .clone()
            .unwrap_or_else(|| format!("{} <参数>", manifest.triggers[0]));
        let plugin = Self::new(
            sandbox,
            manifest.id.clone(),
            manifest.name.clone(),
            manifest.description.clone(),
            LuaScript {
                keywords: manifest.triggers.clone(),
                usage,
                context: manifest.context,
                icon: if manifest.icon.is_empty() { "🧩".to_string() } else { manifest.icon.clone() },
                source: source.into(),
            },
        );
        // manifest 声明的 settings 默认值注入插件私有配置（settings.get 宿主 API 可读）
        for def in &manifest.settings {
            if let Some(default) = &def.default {
                let value = match default {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                plugin.settings.lock().insert(def.key.clone(), value);
            }
        }
        Ok(plugin)
    }

    /// 内置：hosts 映射管理
    pub fn hosts(sandbox: Arc<SandboxManager>) -> Self {
        let plugin = Self::new(
            sandbox,
            "cmd-hosts",
            "Hosts 命令",
            "搜索框直接追加/查看 hosts 映射",
            LuaScript {
                keywords: vec!["hosts".to_string()],
                usage: "hosts <IP> <域名>".to_string(),
                context: false,
                icon: "📝".to_string(),
                source: HOSTS_LUA.into(),
            },
        );
        plugin.settings.lock().insert(
            "hosts_path".to_string(),
            "C:\\Windows\\System32\\drivers\\etc\\hosts".to_string(),
        );
        plugin
    }

    /// 内置：锁定工作站
    pub fn lock_workstation(sandbox: Arc<SandboxManager>) -> Self {
        Self::new(
            sandbox,
            "cmd-lock",
            "锁屏命令",
            "搜索框输入 lock 立即锁定 Windows",
            LuaScript {
                keywords: vec!["lock".to_string(), "锁屏".to_string()],
                usage: "lock".to_string(),
                context: false,
                icon: "🔒".to_string(),
                source: LOCK_LUA.into(),
            },
        )
    }

    /// 内置：选中文件 SHA256（上下文命令示例）
    pub fn file_hash(sandbox: Arc<SandboxManager>) -> Self {
        Self::new(
            sandbox,
            "cmd-hash",
            "Hash 命令",
            "对主列表选中的文件计算 SHA256",
            LuaScript {
                keywords: vec!["hash".to_string(), "sha".to_string(), "sha256".to_string()],
                usage: "hash".to_string(),
                context: true,
                icon: "#️⃣".to_string(),
                source: HASH_LUA.into(),
            },
        )
    }

    /// 内置：网页搜索（g/b/bd 等别名经 __keyword 区分搜索引擎）
    pub fn web_search(sandbox: Arc<SandboxManager>) -> Self {
        Self::new(
            sandbox,
            "cmd-web",
            "网页搜索",
            "搜索框直接 Google/Bing/百度搜索",
            LuaScript {
                keywords: vec![
                    "g".to_string(),
                    "google".to_string(),
                    "bing".to_string(),
                    "b".to_string(),
                    "baidu".to_string(),
                    "bd".to_string(),
                    "百度".to_string(),
                ],
                usage: "g <关键词>".to_string(),
                context: false,
                icon: "🔍".to_string(),
                source: WEB_LUA.into(),
            },
        )
    }

    /// 内置：清空回收站
    pub fn empty_recycle_bin(sandbox: Arc<SandboxManager>) -> Self {
        Self::new(
            sandbox,
            "cmd-emptybin",
            "清空回收站",
            "搜索框输入 emptybin 立即清空回收站",
            LuaScript {
                keywords: vec!["emptybin".to_string(), "清空回收站".to_string()],
                usage: "emptybin".to_string(),
                context: false,
                icon: "🗑".to_string(),
                source: EMPTYBIN_LUA.into(),
            },
        )
    }

    /// 内置：系统睡眠
    pub fn system_sleep(sandbox: Arc<SandboxManager>) -> Self {
        Self::new(
            sandbox,
            "cmd-sleep",
            "睡眠命令",
            "搜索框输入 sleep 立即让系统进入睡眠",
            LuaScript {
                keywords: vec!["sleep".to_string(), "睡眠".to_string()],
                usage: "sleep".to_string(),
                context: false,
                icon: "🌙".to_string(),
                source: SLEEP_LUA.into(),
            },
        )
    }

    /// 插件私有配置覆写（测试注入 hosts_path 等；第三方脚本加载落地后供安装器使用）
    #[allow(dead_code)] // 当前仅测试消费
    pub fn set_setting(&self, key: impl Into<String>, value: impl Into<String>) {
        self.settings.lock().insert(key.into(), value.into());
    }

    /// 剥离危险全局：脚本不接触系统入口，只能走 ilauncher.* 宿主 API
    fn harden(lua: &Lua) -> Result<()> {
        let globals = lua.globals();
        for name in ["io", "os", "debug", "package", "require", "load", "loadstring", "dofile", "loadfile"] {
            globals.set(name, Value::Nil)?;
        }
        Ok(())
    }

    /// 注入宿主 API + 本次调用的参数，加载脚本源码。
    /// keyword = 用户实际输入并命中的关键字（别名级，脚本据此区分多别名行为）
    fn prepare(
        &self,
        args: &[String],
        selection: Option<&str>,
        keyword: &str,
    ) -> Result<(Lua, Arc<Mutex<RunContext>>)> {
        let lua = Lua::new();
        Self::harden(&lua)?;

        let run_ctx = Arc::new(Mutex::new(RunContext::default()));

        // ── ilauncher.settings ────────────────────────────────────────────
        let settings_tbl = lua.create_table()?;
        let settings = self.settings.clone();
        settings_tbl.set(
            "get",
            lua.create_function(move |_, key: String| -> mlua::Result<Option<String>> {
                Ok(settings.lock().get(&key).cloned())
            })?,
        )?;

        // ── ilauncher.shell ───────────────────────────────────────────────
        let shell_tbl = lua.create_table()?;
        let sandbox = self.sandbox.clone();
        let plugin_id = self.metadata.id.clone();
        shell_tbl.set(
            "run",
            lua.create_function(move |_, (program, args): (String, Option<Vec<String>>)| -> mlua::Result<String> {
                sandbox
                    .check_permission(&plugin_id, &PluginPermission::ExecuteProgram)
                    .map_err(|e| mlua::Error::RuntimeError(format!("{e:#}")))?;
                let mut cmd = std::process::Command::new(&program);
                if let Some(args) = args {
                    cmd.args(args);
                }
                let output = cmd.output().map_err(|e| mlua::Error::RuntimeError(format!("exec {program}: {e}")))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    return Err(mlua::Error::RuntimeError(format!("{program} 退出码 {:?}: {stderr}", output.status.code())));
                }
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            })?,
        )?;

        // ── ilauncher.file ────────────────────────────────────────────────
        let file_tbl = lua.create_table()?;
        let mk_file_fn = |lua: &Lua,
                          sandbox: &Arc<SandboxManager>,
                          plugin_id: &str,
                          f: fn(&SandboxManager, &str, &std::path::Path, Option<&str>) -> Result<String>|
         -> Result<mlua::Function> {
            let sandbox = sandbox.clone();
            let plugin_id = plugin_id.to_string();
            Ok(lua.create_function(move |_, (path, content): (String, Option<String>)| {
                let result = f(&sandbox, &plugin_id, std::path::Path::new(&path), content.as_deref());
                result.map_err(|e| mlua::Error::RuntimeError(format!("{e:#}")))
            })?)
        };
        file_tbl.set("read", mk_file_fn(&lua, &self.sandbox, &self.metadata.id, host_file_read)?)?;
        file_tbl.set("write", mk_file_fn(&lua, &self.sandbox, &self.metadata.id, host_file_write)?)?;
        file_tbl.set("append", mk_file_fn(&lua, &self.sandbox, &self.metadata.id, host_file_append)?)?;
        file_tbl.set("sha256", mk_file_fn(&lua, &self.sandbox, &self.metadata.id, host_file_sha256)?)?;

        // ── ilauncher.open / ilauncher.copy（副作用上移 Launcher 层）────────
        let run_ctx_open = run_ctx.clone();
        let open_fn = lua.create_function(move |_, target: String| {
            run_ctx_open.lock().outcome = Some(ExecuteOutcome::Open(target));
            Ok(true)
        })?;
        let sandbox_copy = self.sandbox.clone();
        let plugin_id_copy = self.metadata.id.clone();
        let run_ctx_copy = run_ctx.clone();
        let copy_fn = lua.create_function(move |_, text: String| {
            sandbox_copy
                .check_permission(&plugin_id_copy, &PluginPermission::ClipboardAccess)
                .map_err(|e| mlua::Error::RuntimeError(format!("{e:#}")))?;
            run_ctx_copy.lock().outcome = Some(ExecuteOutcome::Copy(text));
            Ok(true)
        })?;

        let host = lua.create_table()?;
        host.set("settings", settings_tbl)?;
        host.set("shell", shell_tbl)?;
        host.set("file", file_tbl)?;
        host.set("open", open_fn)?;
        host.set("copy", copy_fn)?;
        lua.globals().set("ilauncher", host)?;

        // ── 本次调用的参数 ────────────────────────────────────────────────
        lua.globals().set("__args", lua.create_sequence_from(args.iter().map(|s| s.as_str()))?)?;
        lua.globals().set("__keyword", keyword)?;
        lua.globals().set(
            "__selection",
            match selection {
                Some(s) => Value::String(lua.create_string(s)?),
                None => Value::Nil,
            },
        )?;
        lua.load(self.script.source.as_ref()).exec()?;
        Ok((lua, run_ctx))
    }

    /// 解析 "关键字\u{1}arg1\u{1}arg2" 形式的结果 id
    fn parse_result_id(result_id: &str) -> (String, Vec<String>) {
        let mut parts = result_id.split('\u{1}');
        let kw = parts.next().unwrap_or_default().to_string();
        let args = parts.map(|s| s.to_string()).collect();
        (kw, args)
    }

    /// 编码 "关键字\u{1}arg1\u{1}arg2"。
    /// 注意：编码用户实际命中的关键字（非规范名），execute 侧据此还原 __keyword 别名
    fn encode_result_id(&self, kw: &str, args: &[String]) -> String {
        let mut id = kw.to_string();
        for a in args {
            id.push('\u{1}');
            id.push_str(a);
        }
        id
    }

    fn result(&self, kw: &str, title: impl Into<String>, subtitle: impl Into<String>, args: &[String]) -> QueryResult {
        QueryResult::new(self.encode_result_id(kw, args), title)
            .with_subtitle(subtitle)
            .with_icon(self.script.icon.clone())
            .with_score(COMMAND_SCORE)
            .with_action(PluginAction::default_action("run", "执行"))
    }
}

impl Plugin for LuaCommandPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let q = ctx.search.trim();
        let (kw, rest) = match q.find(char::is_whitespace) {
            Some(i) => (q[..i].trim(), q[i..].trim()),
            None => (q, ""),
        };
        if kw.is_empty() || !self.script.keywords.iter().any(|k| k.eq_ignore_ascii_case(kw)) {
            return Ok(vec![]);
        }
        let args: Vec<String> = rest.split_whitespace().map(|s| s.to_string()).collect();
        *self.pending_selection.lock() = ctx.selection.clone();

        // 上下文命令未选中文件：只给提示行（执行会再次兜底）
        if self.script.context && ctx.selection.is_none() {
            return Ok(vec![self.result(kw, self.script.usage.clone(), "先在主列表中选中一个文件（↑↓ 选择后输入关键字）", &[])]);
        }

        // 脚本的 preview(args, selection) → "标题", "副标题"；缺失/出错回退用法提示。
        // 注意：mlua 的 Value/Table 引用 Lua 状态，必须在 lua 存活的作用域内解析完，
        // 提取为普通 String 后再离开作用域（否则触发 "Lua instance is destroyed"）
        let parsed = self.prepare(&args, ctx.selection.as_deref(), kw).and_then(|(lua, _)| {
            let v = lua
                .load("local a,b = preview(__args, __selection)\nreturn { title = a, subtitle = b }")
                .eval::<Value>()
                .map_err(|e| anyhow!("preview 调用失败: {e}"))?;
            match v {
                Value::Table(t) => {
                    let title = t
                        .get::<Option<String>>("title")?
                        .unwrap_or_else(|| self.script.usage.to_string());
                    let subtitle = t.get::<Option<String>>("subtitle")?.unwrap_or_default();
                    Ok((title, subtitle))
                }
                _ => Ok((self.script.usage.to_string(), String::new())),
            }
        });
        let (title, subtitle) =
            parsed.unwrap_or_else(|e: anyhow::Error| (self.script.usage.to_string(), format!("⚠ {e:#}")));

        Ok(vec![self.result(kw, title, subtitle, &args)])
    }

    fn execute(&self, result_id: &str, _action_id: &str) -> Result<ExecuteOutcome> {
        let (kw, args) = Self::parse_result_id(result_id);
        if !self.script.keywords.iter().any(|k| k.eq_ignore_ascii_case(&kw)) {
            return Err(anyhow!("Unknown command: {kw}"));
        }
        let selection = self.pending_selection.lock().clone();
        let (lua, run_ctx) = self.prepare(&args, selection.as_deref(), &kw)?;
        let message = lua
            .load("return run(__args, __selection)")
            .eval::<Value>()
            .context("run 调用失败")?;
        let message = match message {
            Value::String(s) => match s.to_str() {
                Ok(b) => b.to_string(),
                Err(_) => String::new(),
            },
            _ => String::new(),
        };
        let outcome = run_ctx.lock().outcome.take();
        Ok(outcome.unwrap_or(ExecuteOutcome::Notify(message)))
    }
}

// ── 宿主文件操作（每个函数内部先走沙盒权限检查，检查即审计）────────────────────

fn host_file_read(sandbox: &SandboxManager, plugin_id: &str, path: &std::path::Path, _content: Option<&str>) -> Result<String> {
    sandbox.check_permission(plugin_id, &PluginPermission::FileSystemRead(path.to_path_buf()))?;
    std::fs::read_to_string(path).map_err(|e| anyhow!("读取 {} 失败: {e}", path.display()))
}

fn host_file_write(sandbox: &SandboxManager, plugin_id: &str, path: &std::path::Path, content: Option<&str>) -> Result<String> {
    sandbox.check_permission(plugin_id, &PluginPermission::FileSystemWrite(path.to_path_buf()))?;
    let content = content.ok_or_else(|| anyhow!("file.write 需要内容参数"))?;
    std::fs::write(path, content).map_err(|e| anyhow!("写入 {} 失败: {e}", path.display()))?;
    Ok(String::new())
}

fn host_file_append(sandbox: &SandboxManager, plugin_id: &str, path: &std::path::Path, content: Option<&str>) -> Result<String> {
    sandbox.check_permission(plugin_id, &PluginPermission::FileSystemWrite(path.to_path_buf()))?;
    let content = content.ok_or_else(|| anyhow!("file.append 需要内容参数"))?;
    use std::io::Write as _;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| anyhow!("打开 {} 失败: {e}", path.display()))?;
    f.write_all(content.as_bytes()).map_err(|e| anyhow!("追加 {} 失败: {e}", path.display()))?;
    Ok(String::new())
}

fn host_file_sha256(sandbox: &SandboxManager, plugin_id: &str, path: &std::path::Path, _content: Option<&str>) -> Result<String> {
    use sha2::Digest as _;
    sandbox.check_permission(plugin_id, &PluginPermission::FileSystemRead(path.to_path_buf()))?;
    let data = std::fs::read(path).map_err(|e| anyhow!("读取 {} 失败: {e}", path.display()))?;
    Ok(hex_lower(&sha2::Sha256::digest(&data)))
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    out
}

// ── 内置脚本 ─────────────────────────────────────────────────────────────────

const HOSTS_LUA: &str = r#"
function preview(args, selection)
  if #args == 0 then
    return "hosts <IP> <域名>", "查看 hosts 文件；追加映射：hosts <IP> <域名>"
  elseif #args < 2 then
    return "hosts <IP> <域名>", "参数不足：需要 IP 和域名两个参数"
  end
  return "追加 hosts 映射", args[1] .. " → " .. args[2]
end

function run(args, selection)
  local path = ilauncher.settings.get("hosts_path")
  if #args == 0 then
    return ilauncher.file.read(path)
  end
  if #args < 2 then
    return "用法: hosts <IP> <域名>"
  end
  local line = args[1] .. " " .. args[2]
  ilauncher.file.append(path, line .. "\n")
  return "已追加: " .. line
end
"#;

const LOCK_LUA: &str = r#"
function preview(args, selection)
  return "锁定工作站", "立即锁定 Windows（等价 Win+L）"
end

function run(args, selection)
  ilauncher.shell.run("rundll32.exe", {"user32.dll,LockWorkStation"})
  return "已锁定"
end
"#;

const HASH_LUA: &str = r#"
function preview(args, selection)
  if selection == nil then
    return "hash", "先在主列表中选中一个文件（↑↓ 选择后输入 hash）"
  end
  return "计算 SHA256", selection
end

function run(args, selection)
  if selection == nil then
    return "请先选中文件"
  end
  return ilauncher.file.sha256(selection)
end
"#;

const WEB_LUA: &str = r#"
local ENGINES = {
  google = { name = "Google", url = "https://www.google.com/search?q=" },
  bing   = { name = "Bing",   url = "https://www.bing.com/search?q=" },
  baidu  = { name = "百度",   url = "https://www.baidu.com/s?wd=" },
}

local function resolve()
  local kw = string.lower(__keyword)
  if kw == "g" or kw == "google" then return ENGINES.google end
  if kw == "b" or kw == "bing" then return ENGINES.bing end
  if kw == "bd" or kw == "baidu" or kw == "百度" then return ENGINES.baidu end
  return ENGINES.google
end

function preview(args, selection)
  local e = resolve()
  if #args == 0 then
    return "搜索 " .. e.name, "用法: " .. __keyword .. " <关键词>"
  end
  return "搜索 " .. e.name, table.concat(args, " ")
end

function run(args, selection)
  if #args == 0 then
    return "用法: " .. __keyword .. " <关键词>"
  end
  local e = resolve()
  local q = string.gsub(table.concat(args, " "), " ", "+")
  ilauncher.open(e.url .. q)
  return "搜索 " .. e.name .. ": " .. table.concat(args, " ")
end
"#;

const EMPTYBIN_LUA: &str = r#"
function preview(args, selection)
  return "清空回收站", "立即删除回收站中的全部文件（不可恢复）"
end

function run(args, selection)
  ilauncher.shell.run("powershell.exe", {"-NoProfile", "-Command", "Clear-RecycleBin -Force"})
  return "回收站已清空"
end
"#;

const SLEEP_LUA: &str = r#"
function preview(args, selection)
  return "让系统睡眠", "立即进入睡眠状态"
end

function run(args, selection)
  ilauncher.shell.run("rundll32.exe", {"powrprof.dll,SetSuspendState", "0,1,0"})
  return "已请求系统睡眠"
end
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditLogger;
    use crate::plugin::sandbox::{SandboxConfig, SecurityLevel};
    use crate::plugin::{QueryContext, QueryResult};

    fn sandbox_with(
        plugin_id: &str,
        perms: Vec<PluginPermission>,
        logger: &Arc<Mutex<AuditLogger>>,
    ) -> Arc<SandboxManager> {
        let sandbox = Arc::new(SandboxManager::new(logger.clone()));
        sandbox.register(SandboxConfig {
            plugin_id: plugin_id.to_string(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: Some(perms.into_iter().collect()),
            enabled: true,
            timeout_ms: None,
            max_memory_mb: None,
        });
        sandbox
    }

    fn hosts_plugin(dir: &std::path::Path) -> (LuaCommandPlugin, std::path::PathBuf, Arc<Mutex<AuditLogger>>) {
        let logger = Arc::new(Mutex::new(AuditLogger::in_memory(100)));
        let file = dir.join("hosts");
        std::fs::write(&file, "127.0.0.1 localhost\n").unwrap();
        let p = LuaCommandPlugin::hosts(sandbox_with(
            "cmd-hosts",
            vec![
                PluginPermission::FileSystemRead(dir.to_path_buf()),
                PluginPermission::FileSystemWrite(dir.to_path_buf()),
            ],
            &logger,
        ));
        p.set_setting("hosts_path", file.display().to_string());
        (p, file, logger)
    }

    #[test]
    fn keyword_prefix_matching() {
        let dir = std::env::temp_dir().join("ilauncher_lua_test_kw");
        std::fs::create_dir_all(&dir).unwrap();
        let (p, _, _) = hosts_plugin(&dir);
        assert_eq!(p.query(&QueryContext::new("report")).unwrap().len(), 0);
        // 大小写不敏感
        assert_eq!(p.query(&QueryContext::new("HOSTS")).unwrap().len(), 1);
        assert_eq!(p.query(&QueryContext::new("hosts 1.2.3.4 x.com")).unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn query_result_fields_and_encoding() {
        let dir = std::env::temp_dir().join("ilauncher_lua_test_enc");
        std::fs::create_dir_all(&dir).unwrap();
        let (p, _, _) = hosts_plugin(&dir);
        let rs = p.query(&QueryContext::new("hosts 1.2.3.4 x.com")).unwrap();
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].score, COMMAND_SCORE);
        assert_eq!(rs[0].title, "追加 hosts 映射");
        assert_eq!(rs[0].subtitle, "1.2.3.4 → x.com");
        // result_id 编码关键字 + 参数，execute 可无损还原
        let (kw, args) = LuaCommandPlugin::parse_result_id(&rs[0].id);
        assert_eq!(kw, "hosts");
        assert_eq!(args, vec!["1.2.3.4".to_string(), "x.com".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn execute_hosts_appends_with_audit() {
        let dir = std::env::temp_dir().join("ilauncher_lua_test_run");
        std::fs::create_dir_all(&dir).unwrap();
        let (p, file, logger) = hosts_plugin(&dir);
        let rs = p.query(&QueryContext::new("hosts 10.0.0.1 dev.local")).unwrap();
        let outcome = p.execute(&rs[0].id, "run").unwrap();
        assert_eq!(outcome, ExecuteOutcome::Notify("已追加: 10.0.0.1 dev.local".to_string()));
        let content = std::fs::read_to_string(&file).unwrap();
        assert!(content.contains("10.0.0.1 dev.local"));
        // 沙盒审计：文件写事件已落管道
        assert!(logger.lock().len() >= 1, "文件写权限检查应写审计事件");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn execute_denied_without_permission() {
        let dir = std::env::temp_dir().join("ilauncher_lua_test_deny");
        std::fs::create_dir_all(&dir).unwrap();
        // 空权限表：文件写必须被拒绝
        let logger = Arc::new(Mutex::new(AuditLogger::in_memory(100)));
        let p = LuaCommandPlugin::hosts(sandbox_with("cmd-hosts", vec![], &logger));
        p.set_setting("hosts_path", dir.join("hosts").display().to_string());
        let rs = p.query(&QueryContext::new("hosts 1.1.1.1 a.b")).unwrap();
        assert!(p.execute(&rs[0].id, "run").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn lua_globals_stripped() {
        let lua = Lua::new();
        LuaCommandPlugin::harden(&lua).unwrap();
        for name in ["io", "os", "debug", "require", "load"] {
            assert!(lua.globals().get::<Value>(name).unwrap() == Value::Nil, "{name} 应已被剥离");
        }
    }

    #[test]
    fn context_command_requires_selection() {
        let sandbox = Arc::new(SandboxManager::default());
        let p = LuaCommandPlugin::file_hash(sandbox);
        // 无选中：提示行（仍可执行，run 兜底）
        let rs = p.query(&QueryContext::new("hash")).unwrap();
        assert_eq!(rs.len(), 1);
        assert!(rs[0].subtitle.contains("选中"));
        // 有选中：显示文件路径
        let ctx = QueryContext { selection: Some("C:\\demo\\a.txt".to_string()), ..QueryContext::new("hash") };
        let rs = p.query(&ctx).unwrap();
        assert_eq!(rs[0].subtitle, "C:\\demo\\a.txt");
    }

    #[test]
    fn context_command_execute_sha256() {
        let dir = std::env::temp_dir().join("ilauncher_lua_test_sha");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.txt");
        std::fs::write(&file, "abc").unwrap();
        // cmd-hash 在 manager 中以 System 级注册（任意盘读取，审计保留）；
        // 单测复刻该配置
        let sandbox = Arc::new(SandboxManager::new(Arc::new(Mutex::new(
            AuditLogger::in_memory(100),
        ))));
        sandbox.register(SandboxConfig {
            plugin_id: "cmd-hash".to_string(),
            security_level: SecurityLevel::System,
            custom_permissions: None,
            enabled: false,
            timeout_ms: None,
            max_memory_mb: None,
        });
        let p = LuaCommandPlugin::file_hash(sandbox);
        let ctx = QueryContext {
            selection: Some(file.display().to_string()),
            ..QueryContext::new("hash")
        };
        let rs: Vec<QueryResult> = p.query(&ctx).unwrap();
        let outcome = p.execute(&rs[0].id, "run").unwrap();
        assert_eq!(
            outcome,
            ExecuteOutcome::Notify(
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string()
            )
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn web_search_alias_keyword_injection() {
        // Restricted + 空权限：web 命令只用 ilauncher.open（不经权限检查）
        let logger = Arc::new(Mutex::new(AuditLogger::in_memory(100)));
        let p = LuaCommandPlugin::web_search(sandbox_with("cmd-web", vec![], &logger));

        // 规范别名 g → Google
        let rs = p.query(&QueryContext::new("g rust")).unwrap();
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].title, "搜索 Google");
        let outcome = p.execute(&rs[0].id, "run").unwrap();
        assert_eq!(
            outcome,
            ExecuteOutcome::Open("https://www.google.com/search?q=rust".to_string())
        );

        // 别名 bd → 百度（execute 侧 __keyword 还原自 result_id）
        let rs = p.query(&QueryContext::new("bd 编译 原理")).unwrap();
        assert_eq!(rs[0].title, "搜索 百度");
        let outcome = p.execute(&rs[0].id, "run").unwrap();
        assert_eq!(
            outcome,
            ExecuteOutcome::Open("https://www.baidu.com/s?wd=编译+原理".to_string())
        );

        // 多词查询空格转 +
        let rs = p.query(&QueryContext::new("bing hello world")).unwrap();
        assert_eq!(rs[0].title, "搜索 Bing");
        let outcome = p.execute(&rs[0].id, "run").unwrap();
        assert_eq!(
            outcome,
            ExecuteOutcome::Open("https://www.bing.com/search?q=hello+world".to_string())
        );

        // 无参数：仅用法提示
        let rs = p.query(&QueryContext::new("google")).unwrap();
        assert_eq!(rs[0].title, "搜索 Google");
        assert!(rs[0].subtitle.contains("用法"));
    }

    #[test]
    fn emptybin_and_sleep_query_only() {
        let logger = Arc::new(Mutex::new(AuditLogger::in_memory(100)));
        let bin = LuaCommandPlugin::empty_recycle_bin(sandbox_with(
            "cmd-emptybin",
            vec![PluginPermission::ExecuteProgram],
            &logger,
        ));
        let rs = bin.query(&QueryContext::new("emptybin")).unwrap();
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].title, "清空回收站");
        let rs = bin.query(&QueryContext::new("清空回收站")).unwrap();
        assert_eq!(rs.len(), 1);

        let logger = Arc::new(Mutex::new(AuditLogger::in_memory(100)));
        let sleep = LuaCommandPlugin::system_sleep(sandbox_with(
            "cmd-sleep",
            vec![PluginPermission::ExecuteProgram],
            &logger,
        ));
        let rs = sleep.query(&QueryContext::new("sleep")).unwrap();
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].title, "让系统睡眠");
        let rs = sleep.query(&QueryContext::new("睡眠")).unwrap();
        assert_eq!(rs.len(), 1);
    }
}
