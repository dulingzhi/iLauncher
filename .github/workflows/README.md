# GitHub Actions 发布流程

本目录包含 iLauncher 的 CI/CD 配置，用于自动构建、发布和生成更新器文件。

## 工作流程

### `release.yml` - 发布工作流

当推送带有 `v*` 标签时自动触发，执行以下步骤：

1. **创建 GitHub Release**（草稿状态）
2. **多平台构建**：
   - Windows (x64)
   - macOS (x64 + ARM64)
   - Linux (x64)
3. **生成 `latest.json`** 用于自动更新
4. **发布 Release**（将草稿转为正式发布）

## 配置步骤

### 1. 生成签名密钥对

```bash
# 生成密钥对
bunx tauri signer generate -w ~/.tauri/ilauncher.key

# 输出示例：
# Your public key:
# dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDFEQ0Y1MDQ1RjE2OTU0ODQKUldTRVZHb...
#
# Your private key saved at: ~/.tauri/ilauncher.key
```

### 2. 配置 GitHub Secrets

前往 `Settings` → `Secrets and variables` → `Actions`，添加以下 secrets：

| Secret Name | Description | Value |
|------------|-------------|-------|
| `TAURI_SIGNING_PRIVATE_KEY` | Tauri 签名私钥 | 从 `~/.tauri/ilauncher.key` 复制完整内容 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 私钥密码 | 如果生成时未设置密码，留空或设为空字符串 |

**读取私钥文件**：
```bash
# Windows (PowerShell)
Get-Content ~/.tauri/ilauncher.key -Raw

# macOS/Linux
cat ~/.tauri/ilauncher.key
```

### 3. 更新 `tauri.conf.json` 中的公钥

将生成的公钥添加到配置文件：

```json
{
  "plugins": {
    "updater": {
      "active": true,
      "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDFEQ0Y1MDQ1RjE2OTU0ODQKUldTRVZHb...",
      "endpoints": [
        "https://github.com/dulingzhi/iLauncher/releases/latest/download/latest.json"
      ]
    }
  }
}
```

## 发布新版本

### 自动发布（推荐）

1. **更新版本号**：
   ```bash
   # 同时更新 package.json 和 tauri.conf.json
   npm version patch  # 或 minor / major
   ```

2. **创建并推送 Git 标签**：
   ```bash
   git add .
   git commit -m "chore: bump version to v0.2.0"
   git tag v0.2.0
   git push origin master --tags
   ```

3. **等待 GitHub Actions 完成**：
   - 访问 `Actions` 页面查看构建进度
   - 构建完成后，Release 会自动发布
   - `latest.json` 会自动生成并上传

### 手动发布

如果需要手动控制发布流程：

1. **本地构建**：
   ```bash
   # 设置签名私钥环境变量
   # Windows (PowerShell)
   $env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content ~/.tauri/ilauncher.key -Raw)
   
   # macOS/Linux
   export TAURI_SIGNING_PRIVATE_KEY=$(cat ~/.tauri/ilauncher.key)
   
   # 构建
   bun tauri build
   ```

2. **生成 `latest.json`**：
   ```bash
   node scripts/generate-updater-json.js 0.2.0 v0.2.0
   ```

3. **创建 GitHub Release**：
   - 前往 `Releases` → `New Release`
   - 创建标签（如 `v0.2.0`）
   - 上传以下文件：
     - `src-tauri/target/release/bundle/nsis/*.nsis.zip` + `.sig`
     - `src-tauri/target/release/bundle/macos/*.app.tar.gz` + `.sig`
     - `src-tauri/target/release/bundle/appimage/*.AppImage.tar.gz` + `.sig`
     - `latest.json`

## 工作流触发方式

### 自动触发（推荐）

```bash
git tag v0.2.0
git push origin v0.2.0
```

### 手动触发

前往 `Actions` → `Release` → `Run workflow`，手动触发工作流。

## 构建产物

每个平台的构建产物：

| Platform | Installer | Signature | Auto-update Archive |
|----------|-----------|-----------|---------------------|
| **Windows** | `.msi`, `.exe` | `.msi.sig`, `.exe.sig` | `.nsis.zip` + `.nsis.zip.sig` |
| **macOS x64** | `.dmg`, `.app` | `.dmg.sig`, `.app.sig` | `.app.tar.gz` + `.app.tar.gz.sig` |
| **macOS ARM** | `.dmg`, `.app` | `.dmg.sig`, `.app.sig` | `.app.tar.gz` + `.app.tar.gz.sig` |
| **Linux** | `.deb`, `.AppImage` | `.deb.sig`, `.AppImage.sig` | `.AppImage.tar.gz` + `.AppImage.tar.gz.sig` |

⚠️ **重要**：自动更新只使用 `.zip`/`.tar.gz` 压缩包，不使用原始安装程序。

## 故障排查

### 1. 签名验证失败

**问题**：构建成功但 `.sig` 文件未生成

**解决方案**：
- 检查 GitHub Secrets 是否正确配置
- 确认私钥内容完整（包括头尾注释）
- 验证私钥密码（如果有）

### 2. 构建失败

**问题**：GitHub Actions 构建报错

**常见原因**：
- 依赖安装失败 → 检查 `package.json`
- Rust 编译错误 → 本地运行 `cargo build`
- 前端构建失败 → 本地运行 `bun build`

### 3. `latest.json` 缺失平台

**问题**：某些平台未包含在 `latest.json` 中

**解决方案**：
- 检查该平台的构建是否成功
- 确认 `.sig` 文件与安装包同名
- 查看 GitHub Actions 日志中的 `generate-updater-json` 步骤

### 4. 更新检查失败

**问题**：应用无法检测到更新

**解决方案**：
- 确认 `latest.json` 可访问：
  ```
  https://github.com/dulingzhi/iLauncher/releases/latest/download/latest.json
  ```
- 检查版本号格式（必须是 `v1.2.3` 格式）
- 确认 `tauri.conf.json` 中的 `endpoints` 配置正确

## 最佳实践

### 版本命名规范

遵循 [Semantic Versioning](https://semver.org/)：

- **主版本** (MAJOR): 不兼容的 API 变更 → `1.0.0` → `2.0.0`
- **次版本** (MINOR): 向下兼容的新功能 → `1.0.0` → `1.1.0`
- **修订版本** (PATCH): 向下兼容的 Bug 修复 → `1.0.0` → `1.0.1`

### Git 标签规范

- 使用 `v` 前缀：`v1.0.0`（不是 `1.0.0`）
- 与 `package.json` 版本号一致
- 包含有意义的 Release Notes

### Release Notes 建议

```markdown
## What's New

### Features
- ✨ New feature 1
- ✨ New feature 2

### Bug Fixes
- 🐛 Fixed bug 1
- 🐛 Fixed bug 2

### Performance
- ⚡ Performance improvement 1

### Breaking Changes
- ⚠️ Breaking change 1

**Full Changelog**: https://github.com/dulingzhi/iLauncher/compare/v0.1.0...v0.2.0
```

## 本地测试更新流程

### 1. 构建旧版本

```bash
# 修改版本号为 0.1.0
vim package.json src-tauri/tauri.conf.json

# 构建
bun tauri build
```

### 2. 创建模拟 Release

```bash
# 构建新版本 (0.2.0)
vim package.json src-tauri/tauri.conf.json
bun tauri build

# 生成 latest.json
node scripts/generate-updater-json.js 0.2.0 v0.2.0

# 创建本地 HTTP 服务器
cd src-tauri/target/release/bundle
python -m http.server 8080
```

### 3. 修改配置指向本地服务器

```json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "http://localhost:8080/latest.json"
      ]
    }
  }
}
```

### 4. 运行旧版本测试更新

运行 `0.1.0` 版本，等待自动更新检测。

## 参考资源

- [Tauri Updater 官方文档](https://v2.tauri.app/plugin/updater/)
- [tauri-action GitHub](https://github.com/tauri-apps/tauri-action)
- [GitHub Actions 文档](https://docs.github.com/en/actions)
- [Semantic Versioning](https://semver.org/)
