# GitHub Actions 发布流程

gpui 通道发版：推送 `v*` 标签 → CI（Windows）构建 → NSIS 打安装包 → minisign 签名 → 生成 `latest.json` → 发布 Release。

## 工作流程

`release.yml` 在推送 `v*` 标签（或手动触发）时执行：

1. 安装 Rust、NSIS（choco）、minisign（cargo install）
2. 用标签号更新 `gpui-app/Cargo.toml` 版本
3. `cargo build --release --features "ilauncher clipboard"`
4. `scripts/pack-gpui.ps1 -SkipBuild -Sign` → `iLauncher_<ver>_x64-setup.exe` + `.sig`
5. `scripts/generate-updater-json.js` → `latest.json`
6. softprops/action-gh-release 上传三个产物

## 配置步骤

### 1. 准备 minisign 私钥

公钥已硬编码在 `gpui-app/src/updater.rs`（`UPDATE_PUBKEY`），私钥持有者在本地签名过历史版本。
若需重新生成密钥对（会切断旧客户端更新，谨慎）：

```powershell
cargo install minisign --locked
minisign -G -p iLauncher.pub -s iLauncher.key
# 把新公钥（.pub 文件内容的 base64）更新到 updater.rs 的 UPDATE_PUBKEY
```

### 2. 配置 GitHub Secret

| Secret | 内容 |
|---|---|
| `MINISIGN_SECRET_KEY_BASE64` | 整个私钥文件的 base64（`[Convert]::ToBase64String([IO.File]::ReadAllBytes("iLauncher.key"))`） |

### 3. 发版

```bash
git tag v0.2.0
git push origin v0.2.0
```

## 手动发布

```powershell
# 本地打包（需 NSIS；加 -Sign 并用 MINISIGN_SECRET_KEY_FILE 指向私钥）
powershell -File scripts/pack-gpui.ps1 -Version 0.2.0
node scripts/generate-updater-json.js 0.2.0 v0.2.0
# 手动创建 Release，上传 setup.exe、setup.exe.sig、latest.json
```

## 协议要点（消费端 gpui-app/src/updater.rs）

- `latest.json` 的 `platforms.windows-x86_64.url` 直接指向 **setup.exe**（下载字节即验签对象，无需 zip 解包）
- `signature` = base64(整个 `.minisig` 文件)
- 安装以 `/SILENT` 启动（安装脚本里已映射为 NSIS silent，被动安装无交互）
- 安装目录与卸载项对齐旧版：`%LOCALAPPDATA%\Programs\iLauncher`，可原地覆盖旧 Tauri 安装
- 内网/测试源：设置环境变量 `ILAUNCHER_UPDATE_URL` 指向自定义 `latest.json`

## 故障排查

- **签名验证失败**：确认 `.sig` 由 `pack-gpui.ps1 -Sign` 生成（不是 `.minisig` 原名）；确认公钥与私钥配对。
- **更新检测不到**：确认 `latest.json` 可访问，版本格式 `v1.2.3`，且大于客户端 `gpui-app/Cargo.toml` 版本。
- **覆盖安装失败**：安装脚本会 taskkill `iLauncher.exe` / `ilauncher-gpui.exe`，确认安装时旧进程已退出。
