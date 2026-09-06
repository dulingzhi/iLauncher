# iLauncher Lua 命令插件开发指南

iLauncher 的插件命令系统类似 Listary：在搜索框输入关键字即可触发命令（如 `hosts 127.0.0.1 dev.local`），也可以对主列表中选中的文件执行上下文命令（如 `hash` 计算 SHA256）。

插件用 **Lua 5.4**（[mlua](https://github.com/mlua-rs/mlua)，沙箱模式）编写，打包为 `.ilp`（zip）后由 iLauncher 插件页安装。本文档面向插件作者。

---

## 1. 快速开始

最小插件 = 一个 `manifest.json` + 一个 Lua 入口文件：

```
com.example.hello/
├── manifest.json
└── main.lua
```

`manifest.json`：

```json
{
  "id": "com.example.hello",
  "name": "Hello 命令",
  "version": "1.0.0",
  "description": "向状态栏问好（最小示例）",
  "author": { "name": "you" },
  "license": "MIT",
  "icon": "👋",
  "engine": { "type": "lua", "entry": "main.lua", "runtime_version": "5.4" },
  "triggers": ["hello"],
  "permissions": [],
  "sandbox": { "level": "restricted", "timeout_ms": 1000, "max_memory_mb": 50 }
}
```

`main.lua`：

```lua
function preview(args, selection)
  return "Hello, iLauncher!", "无参数，回车执行"
end

function run(args, selection)
  return "你好，世界！"
end
```

打包（PowerShell，在插件目录的上一级执行）：

```powershell
Compress-Archive -Path com.example.hello -DestinationPath com.example.hello.ilp
```

然后在 iLauncher **设置 → 插件** 页安装该 `.ilp` 即可。完整可参考仓库内示例：`examples/lua-plugins/com.ilauncher-demo.upper`。

---

## 2. manifest.json 字段

| 字段 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `id` | string | ✓ | 全局唯一，**至少 3 段**点分（如 `com.author.name`），每段非空 |
| `name` | string | ✓ | 显示名 |
| `version` | string | ✓ | 语义化版本 |
| `description` | string | ✓ | 一句话描述（列表页副标题） |
| `author` | object | ✓ | `{ "name": ..., "email"?: ..., "url"?: ... }` |
| `license` | string | ✓ | SPDX 标识，如 `MIT` |
| `engine` | object | ✓ | 见下 |
| `triggers` | string[] | ✓（Lua） | 触发关键字，**至少 1 个**；全部大小写不敏感，可含中文 |
| `permissions` | string[] | ✓ | 权限声明，见第 4 节；不需要能力给空数组 |
| `sandbox` | object | ✓ | 沙盒档位，见下 |
| `icon` | string | | 列表图标（emoji 或文本），缺省 `🧩` |
| `usage` | string | | 用法提示（参数不足时的兜底标题），缺省 `triggers[0] <参数>` |
| `context` | bool | | `true` = 上下文命令：对主列表选中文件操作（选中路径注入 `selection`），缺省 `false` |
| `settings` | object[] | | 插件配置项（`settings.get` 可读），见下 |
| `homepage` / `repository` / `keywords` / `dependencies` / `changelog` | — | | 预留元数据 |

### engine

```json
{ "type": "lua", "entry": "main.lua", "runtime_version": "5.4" }
```

- `type` 目前仅 `lua` 为可运行引擎（其余类型会被拒绝安装）。
- `entry` = 入口 Lua 文件（相对包根）。
- `runtime_version` 固定 `5.4`。

### sandbox.level 四档

| level | 运行时等级 | 含义 |
|---|---|---|
| `none` | System | 无限制（安装页会醒目标注，慎用） |
| `basic` | Trusted | 信任插件，权限检查放行但仍记审计 |
| `restricted` | Restricted | **推荐**：只允许 `permissions` 白名单中的能力 |
| `strict` | Sandboxed | 最严，仅沙箱内资源 |

- `timeout_ms` / `max_memory_mb`：单次调用超时与内存上限。

### settings（可选配置项）

```json
"settings": [
  {
    "key": "api_url",
    "label": "API 地址",
    "description": "请求的后端地址",
    "required": false,
    "secret": false,
    "default": "https://example.com"
  }
]
```

`default` 在安装时注入插件私有配置，脚本内 `ilauncher.settings.get("api_url")` 读取。

---

## 3. 脚本协议

每个入口文件定义两个**全局函数**：

```lua
function preview(args, selection) return "标题", "副标题" end
function run(args, selection) return "状态栏反馈文本" end
```

- `preview`：**可选**。返回两个字符串（标题 / 副标题），展示在搜索结果行。缺失或出错时回退到 `usage`。
- `run`：**必需**。用户回车执行时调用；返回字符串会显示在启动器状态栏。
  - 若调用 `ilauncher.open(...)` 或 `ilauncher.copy(...)`，返回值仅作提示，实际打开/复制由启动器接管。

### 注入参数

| 变量 | 类型 | 说明 |
|---|---|---|
| `args` | table (array) | 关键字之后按空白切分的参数（前缀命令） |
| `selection` | string / nil | 上下文命令选中的文件绝对路径；无前缀命令时为 `nil` |
| `__keyword` | string | 用户**实际输入并命中**的关键字（别名级）。脚本据此区分多别名行为（如 `g`/`bd` 走不同搜索引擎）。query 与 execute 阶段均已注入，取别名时建议 `string.lower(__keyword)` 后比较 |

### 触发规则

- 前缀命令：查询以 `trigger + 空白` 开头（或恰为 trigger 本身）即命中，其余部分作为参数。命令结果置顶显示（得分高于计算器）。
- 上下文命令（`context: true`）：需要主列表当前选中文件；未选中时只显示提示行，执行时 `selection` 为 `nil`，脚本应兜底。
- 关键字匹配大小写不敏感（`Hosts` / `HOSTS` 均可），中文别名直接可用。

---

## 4. 宿主 API（`ilauncher.*`）

脚本**没有** `io` / `os` / `debug` / `package` / `require` / `load` / `dofile` 等全局（运行前已剥离），系统能力只能通过以下宿主 API：

### `ilauncher.settings.get(key) -> string | nil`

读取插件私有配置（manifest `settings` 的默认值已注入）。

### `ilauncher.shell.run(program, args?) -> string`

执行外部程序并等待结束，返回标准输出（去首尾空白）。非零退出码抛错。需要 `system:execute` 权限。

```lua
ilauncher.shell.run("powershell.exe", {"-NoProfile", "-Command", "Get-Date"})
```

### `ilauncher.file.read(path) / write(path, content) / append(path, content) -> string`

读写文本文件（`read` 返回内容，`write`/`append` 返回空串）。需要 `filesystem:read:<路径前缀>` / `filesystem:write:<路径前缀>` 权限，**路径前缀匹配**。

### `ilauncher.file.sha256(path) -> string`

计算文件 SHA256（hex 小写）。需要 `filesystem:read:<路径前缀>` 权限。

### `ilauncher.open(target)`

请求启动器打开目标（URL / 文件 / 程序）。**不经权限检查**，实际打开由启动器层执行。

### `ilauncher.copy(text)`

复制文本到剪贴板。需要 `clipboard:write`（`clipboard:read` 同样映射到剪贴板权限）。

### 权限声明对照表（manifest `permissions`）

| 声明 | 能力 |
|---|---|
| `network:all` / `network:<域名>` | 网络访问范围（保留，当前 Lua API 未直接暴露网络） |
| `filesystem:read:<路径前缀>` | 读该前缀下文件（如 `C:\\Temp`） |
| `filesystem:write:<路径前缀>` | 写该前缀下文件 |
| `clipboard:read` / `clipboard:write` | 剪贴板 |
| `system:info` | 系统信息读取（保留） |
| `system:execute` | `ilauncher.shell.run` 执行外部程序 |

每次权限检查都会写入审计日志（设置页可查）。

---

## 5. 打包与安装

```powershell
# 在插件目录的上一级执行，保证包内第一层就是 manifest.json
Compress-Archive -Path com.example.hello -DestinationPath com.example.hello.ilp
```

- `.ilp` 本质是 zip，根目录必须直接包含 `manifest.json` 与 `engine.entry` 指定的文件。
- 安装入口：iLauncher **设置 → 插件 → 安装**，或放入 `%LOCALAPPDATA%\iLauncher\plugins` 后重启。
- 同 `id` 插件已安装时需先卸载再装新版本。
- 安装后可在插件页单独禁用；禁用即不再参与搜索。

---

## 6. 内置命令参考（可对照学习）

| 关键字 | 说明 |
|---|---|
| `hosts <IP> <域名>` | 查看 / 追加 hosts 映射 |
| `lock` / `锁屏` | 锁定工作站 |
| `hash` / `sha` / `sha256`（上下文） | 选中文件 SHA256 |
| `g` `google` / `b` `bing` / `bd` `baidu` `百度` | 网页搜索（多别名经 `__keyword` 区分引擎） |
| `emptybin` / `清空回收站` | 清空回收站 |
| `sleep` / `睡眠` | 系统睡眠 |

多别名命令（网页搜索）的完整源码见 `crates/ilauncher-gpui/src/plugin/lua_cmd.rs` 内 `WEB_LUA` 常量。
