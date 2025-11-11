# iLauncher 自动更新设置指南

## 概述

iLauncher 使用 Tauri Updater 插件实现自动更新功能。更新系统支持：

- ✅ 自动检查更新（启动后 5 秒）
- ✅ 手动检查更新（设置页面按钮）
- ✅ 下载进度显示
- ✅ 自动安装并重启
- ✅ Release Notes 展示
- ✅ 签名验证（需配置公钥）

## 工作原理

### 1. 更新流程

```
启动应用 → 5秒延迟 → 静默检查更新
                      ↓
                  发现新版本
                      ↓
              显示更新对话框
                      ↓
              用户确认更新
                      ↓
            下载 + 进度显示
                      ↓
              验证签名
                      ↓
            安装 + 重启应用
```

### 2. 更新元数据格式

Tauri Updater 需要一个 `latest.json` 文件，格式如下：

```json
{
  "version": "0.2.0",
  "notes": "新功能：开机自启、自动更新\n修复：若干 Bug",
  "pub_date": "2025-01-11T12:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "",
      "url": "https://github.com/dulingzhi/iLauncher/releases/download/v0.2.0/iLauncher_0.2.0_x64-setup.nsis.zip"
    },
    "darwin-x86_64": {
      "signature": "",
      "url": "https://github.com/dulingzhi/iLauncher/releases/download/v0.2.0/iLauncher_0.2.0_x64.app.tar.gz"
    },
    "darwin-aarch64": {
      "signature": "",
      "url": "https://github.com/dulingzhi/iLauncher/releases/download/v0.2.0/iLauncher_0.2.0_aarch64.app.tar.gz"
    },
    "linux-x86_64": {
      "signature": "",
      "url": "https://github.com/dulingzhi/iLauncher/releases/download/v0.2.0/iLauncher_0.2.0_amd64.AppImage.tar.gz"
    }
  }
}
```

## 配置步骤

### 1. 生成签名密钥对（推荐但可选）

```bash
# 安装 Tauri CLI（如果还没安装）
bun add -D @tauri-apps/cli

# 生成密钥对
bunx tauri signer generate -w ~/.tauri/myapp.key
```

输出示例：
```
Your public key:
dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25lZCBieSBhY3RpdmF0ZS1hcHBzCm...

Your private key:
# 保存在 ~/.tauri/myapp.key
```

### 2. 配置 tauri.conf.json

将公钥添加到配置文件：

```json
{
  "plugins": {
    "updater": {
      "active": true,
      "dialog": true,
      "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25lZCBieSBhY3RpdmF0ZS1hcHBzCm...",
      "endpoints": [
        "https://github.com/dulingzhi/iLauncher/releases/latest/download/latest.json"
      ]
    }
  }
}
```

### 3. 构建并签名 Release

```bash
# Windows
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content ~/.tauri/myapp.key -Raw)
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""  # 如果设置了密码
bun tauri build

# macOS/Linux
export TAURI_SIGNING_PRIVATE_KEY=$(cat ~/.tauri/myapp.key)
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""  # 如果设置了密码
bun tauri build
```

构建完成后，会生成：
- 安装包（如 `.exe`, `.dmg`, `.AppImage`）
- 签名文件（`.sig`）

### 4. 创建 GitHub Release

#### 手动创建（推荐用于测试）

1. 前往 GitHub 仓库 → Releases → New Release
2. 创建新标签（如 `v0.2.0`）
3. 上传文件：
   - 安装包（压缩为 `.zip` 或 `.tar.gz`）
   - 签名文件（`.sig`）
   - `latest.json` 元数据文件

#### 自动化 GitHub Actions（推荐用于生产）

创建 `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    strategy:
      matrix:
        platform: [windows-latest, macos-latest, ubuntu-latest]
    runs-on: ${{ matrix.platform }}

    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Bun
        uses: oven-sh/setup-bun@v1
      
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Install dependencies
        run: bun install
      
      - name: Build Tauri App
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        run: bun tauri build
      
      - name: Upload Release Assets
        uses: softprops/action-gh-release@v1
        with:
          files: |
            src-tauri/target/release/bundle/**/*.exe
            src-tauri/target/release/bundle/**/*.dmg
            src-tauri/target/release/bundle/**/*.AppImage
            src-tauri/target/release/bundle/**/*.sig
```

### 5. 生成 latest.json

可以手动创建，也可以使用脚本自动生成：

```javascript
// scripts/generate-update-json.js
import fs from 'fs';
import path from 'path';

const version = process.env.VERSION || '0.2.0';
const releaseTag = `v${version}`;
const baseUrl = `https://github.com/dulingzhi/iLauncher/releases/download/${releaseTag}`;

const updateInfo = {
  version,
  notes: "新版本更新说明",
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature: fs.readFileSync('src-tauri/target/release/bundle/nsis/iLauncher_0.2.0_x64-setup.nsis.zip.sig', 'utf8').trim(),
      url: `${baseUrl}/iLauncher_${version}_x64-setup.nsis.zip`
    },
    // ... 其他平台
  }
};

fs.writeFileSync('latest.json', JSON.stringify(updateInfo, null, 2));
console.log('✓ latest.json generated');
```

## 测试更新功能

### 本地测试

1. **修改版本号**：
   ```json
   // package.json
   "version": "0.1.0"
   
   // tauri.conf.json
   "version": "0.1.0"
   ```

2. **构建旧版本**：
   ```bash
   bun tauri build
   ```

3. **创建模拟更新服务器**：
   ```bash
   # 创建 latest.json（模拟新版本 0.2.0）
   # 放在 HTTP 服务器上（如 http://localhost:8080/latest.json）
   
   # 修改 tauri.conf.json 指向本地服务器
   "endpoints": ["http://localhost:8080/latest.json"]
   ```

4. **运行应用并测试**：
   - 启动应用后等待 5 秒
   - 或点击 Settings → Advanced → Check for Updates

### 生产环境测试

1. 部署 `v0.1.0` 版本
2. 创建 `v0.2.0` Release（含 `latest.json`）
3. 运行 `v0.1.0` 应用
4. 应自动检测到 `v0.2.0` 更新

## 常见问题

### 1. 签名验证失败

**错误**: `Invalid signature`

**解决方案**:
- 确认公钥正确配置在 `tauri.conf.json`
- 确认构建时使用了正确的私钥
- 检查 `.sig` 文件与安装包匹配

### 2. 无法下载更新

**错误**: `Failed to download update`

**解决方案**:
- 检查 `latest.json` URL 是否可访问
- 检查安装包 URL 是否正确
- 确认文件已压缩为 `.zip` 或 `.tar.gz`

### 3. 更新检查失败

**错误**: `Update check failed`

**解决方案**:
- 检查网络连接
- 确认 `endpoints` URL 返回有效 JSON
- 查看日志：`AppData\Local\iLauncher\logs\ilauncher.log`

### 4. 跳过签名验证（仅用于开发/测试）

```json
// tauri.conf.json
{
  "plugins": {
    "updater": {
      "active": true,
      "dialog": true,
      "pubkey": "",  // 空字符串 = 跳过签名验证
      "endpoints": [...]
    }
  }
}
```

⚠️ **警告**: 生产环境必须启用签名验证！

## 更新策略建议

### 自动更新频率

- **启动检查**: 应用启动后 5 秒（已实现）
- **定期检查**: 可选，每 24 小时检查一次
- **手动检查**: 设置页面按钮（已实现）

### 版本策略

- **主版本**: 重大更新（如 1.0.0 → 2.0.0）
- **次版本**: 新功能（如 1.0.0 → 1.1.0）
- **修订版本**: Bug 修复（如 1.0.0 → 1.0.1）

### 强制更新

修改 `UpdateChecker.tsx` 实现强制更新：

```typescript
if (update?.available) {
  // 检查是否为强制更新（如安全补丁）
  const isCritical = update.body?.includes('[CRITICAL]');
  
  const message = isCritical
    ? 'Critical security update available. This update will be installed automatically.'
    : 'New version available. Do you want to update now?';
  
  const shouldUpdate = isCritical || confirm(message);
  
  if (shouldUpdate) {
    // ... 下载并安装
  }
}
```

## 参考资源

- [Tauri Updater 官方文档](https://v2.tauri.app/plugin/updater/)
- [GitHub Releases API](https://docs.github.com/en/rest/releases)
- [Semantic Versioning](https://semver.org/)

## 实现清单

- ✅ 添加 tauri-plugin-updater 依赖
- ✅ 配置 tauri.conf.json
- ✅ 创建 UpdateChecker 组件
- ✅ 集成到 Settings 页面
- ✅ 启动时自动检查（5秒延迟）
- ✅ 手动检查按钮
- ✅ 下载进度显示
- ✅ Toast 通知
- ⏳ 生成签名密钥对（需手动）
- ⏳ 配置 GitHub Actions（可选）
- ⏳ 创建首个 Release（需手动）

## 下一步

1. 生成签名密钥对
2. 将公钥添加到 `tauri.conf.json`
3. 创建 GitHub Release 工作流
4. 发布第一个正式版本（v0.2.0）
5. 测试完整更新流程
