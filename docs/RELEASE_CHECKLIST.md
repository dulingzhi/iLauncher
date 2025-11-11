# 发布清单 - Quick Reference

## 📋 发布前检查清单

- [ ] 所有测试通过
- [ ] 更新 CHANGELOG.md
- [ ] 更新版本号（`package.json` + `tauri.conf.json`）
- [ ] 提交所有代码变更
- [ ] 推送到 GitHub

## 🚀 发布步骤（自动）

```bash
# 1. 更新版本号
npm version patch  # 或 minor / major

# 2. 创建并推送标签
git push origin master --tags

# 3. 等待 GitHub Actions 完成（约 20-30 分钟）
# 访问 https://github.com/dulingzhi/iLauncher/actions

# 4. 检查 Release 页面
# 访问 https://github.com/dulingzhi/iLauncher/releases
```

## 🛠️ 手动发布步骤

### 1. 设置签名密钥

```powershell
# Windows
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content ~/.tauri/ilauncher.key -Raw)
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
```

```bash
# macOS/Linux
export TAURI_SIGNING_PRIVATE_KEY=$(cat ~/.tauri/ilauncher.key)
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
```

### 2. 构建所有平台

```bash
# 本地构建（仅当前平台）
bun tauri build

# 跨平台构建需要使用 GitHub Actions 或虚拟机
```

### 3. 生成 latest.json

```bash
bun run generate-updater-json 0.2.0 v0.2.0
```

### 4. 创建 GitHub Release

前往 https://github.com/dulingzhi/iLauncher/releases/new

上传文件：
- Windows: `*.nsis.zip` + `.sig`
- macOS: `*.app.tar.gz` + `.sig`（x64 + ARM64）
- Linux: `*.AppImage.tar.gz` + `.sig`
- `latest.json`

## 🔍 验证清单

### 发布后验证

- [ ] Release 页面所有文件已上传
- [ ] `latest.json` 可访问
- [ ] 各平台安装包可下载
- [ ] 签名文件存在

### 更新测试

- [ ] 旧版本能检测到更新
- [ ] 下载进度正常显示
- [ ] 更新安装成功
- [ ] 应用重启后版本正确

## 📦 文件清单

每个 Release 应包含：

| 文件 | 必需 | 用途 |
|-----|------|-----|
| `iLauncher_x.x.x_x64-setup.nsis.zip` | ✅ | Windows 自动更新 |
| `iLauncher_x.x.x_x64-setup.nsis.zip.sig` | ✅ | Windows 签名 |
| `iLauncher_x.x.x_x64.app.tar.gz` | ✅ | macOS x64 自动更新 |
| `iLauncher_x.x.x_x64.app.tar.gz.sig` | ✅ | macOS x64 签名 |
| `iLauncher_x.x.x_aarch64.app.tar.gz` | ✅ | macOS ARM 自动更新 |
| `iLauncher_x.x.x_aarch64.app.tar.gz.sig` | ✅ | macOS ARM 签名 |
| `iLauncher_x.x.x_amd64.AppImage.tar.gz` | ✅ | Linux 自动更新 |
| `iLauncher_x.x.x_amd64.AppImage.tar.gz.sig` | ✅ | Linux 签名 |
| `latest.json` | ✅ | 更新元数据 |
| `iLauncher_x.x.x_x64-setup.exe` | 可选 | Windows 安装程序 |
| `iLauncher_x.x.x_x64.msi` | 可选 | Windows MSI |
| `iLauncher_x.x.x_x64.dmg` | 可选 | macOS 安装包 |
| `iLauncher_x.x.x_amd64.deb` | 可选 | Linux DEB |
| `iLauncher_x.x.x_amd64.AppImage` | 可选 | Linux AppImage |

## 🔐 GitHub Secrets 配置

| Secret | 值 | 如何获取 |
|--------|----|---------| 
| `TAURI_SIGNING_PRIVATE_KEY` | 私钥内容 | `Get-Content ~/.tauri/ilauncher.key -Raw` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 私钥密码 | 生成密钥时设置的密码（留空如未设置）|

## 📝 版本号规范

| 类型 | 示例 | 何时使用 |
|-----|------|---------|
| MAJOR | `1.0.0` → `2.0.0` | 不兼容的 API 变更 |
| MINOR | `1.0.0` → `1.1.0` | 向下兼容的新功能 |
| PATCH | `1.0.0` → `1.0.1` | 向下兼容的 Bug 修复 |

## ⚠️ 常见错误

### 签名验证失败
- 检查私钥是否正确配置
- 确认公钥已更新到 `tauri.conf.json`

### latest.json 404
- 确认 Release 已发布（不是草稿）
- 检查文件名是否为 `latest.json`

### 某平台未更新
- 检查该平台的构建是否成功
- 确认 `.sig` 文件存在

## 🔗 快速链接

- [GitHub Actions](https://github.com/dulingzhi/iLauncher/actions)
- [Releases](https://github.com/dulingzhi/iLauncher/releases)
- [latest.json](https://github.com/dulingzhi/iLauncher/releases/latest/download/latest.json)
- [Tauri Updater 文档](https://v2.tauri.app/plugin/updater/)
