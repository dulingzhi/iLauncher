#!/usr/bin/env node

/**
 * 生成 latest.json（gpui 更新通道）
 *
 * 消费契约见 crates/ilauncher-gpui/src/updater.rs：
 *   - url 直接指向 iLauncher_<ver>_x64-setup.exe（下载字节即安装程序，minisign 直接验签）
 *   - signature = base64(整个 .minisig 文件内容)，由 scripts/pack-gpui.ps1 -Sign 生成
 *
 * 用法：node scripts/generate-updater-json.js <version> <release-tag>
 * 示例：node scripts/generate-updater-json.js 0.2.0 v0.2.0
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const version = process.argv[2] || process.env.VERSION || '0.1.0';
const releaseTag = process.argv[3] || process.env.RELEASE_TAG || `v${version}`;
const owner = process.env.GITHUB_OWNER || 'dulingzhi';
const repo = env('GITHUB_REPO', 'iLauncher');
// 产物目录可用 ARTIFACT_DIR 覆盖（CI 里指向下载好的产物）
const artifactDir =
    process.env.ARTIFACT_DIR || path.join(__dirname, '..', 'crates/ilauncher-gpui', 'target', 'release', 'bundle', 'nsis');

function env(name, fallback) {
    return process.env[name] || fallback;
}

const baseUrl = `https://github.com/${owner}/${repo}/releases/download/${releaseTag}`;
const setupName = `iLauncher_${version}_x64-setup.exe`;
const setup = path.join(artifactDir, setupName);

console.log(`Generating latest.json for version ${version} (tag: ${releaseTag})`);
console.log(`Artifacts: ${artifactDir}`);

if (!fs.existsSync(setup)) {
    console.error(`❌ 未找到 ${setup}`);
    console.error('   请先运行：powershell -File scripts/pack-gpui.ps1 [-Sign]');
    process.exit(1);
}

const sigPath = `${setup}.sig`;
if (!fs.existsSync(sigPath)) {
    console.error(`❌ 未找到签名 ${sigPath}（pack-gpui.ps1 加 -Sign 生成）`);
    process.exit(1);
}
const signature = fs.readFileSync(sigPath, 'utf8').trim();

const updateInfo = {
    version: `v${version}`,
    notes: `See release notes on GitHub: https://github.com/${owner}/${repo}/releases/tag/${releaseTag}`,
    pub_date: new Date().toISOString(),
    platforms: {
        'windows-x86_64': {
            signature,
            url: `${baseUrl}/${setupName}`,
        },
    },
};

const outputPath = path.join(__dirname, '..', 'latest.json');
fs.writeFileSync(outputPath, JSON.stringify(updateInfo, null, 2));

console.log(`\n✅ Generated latest.json:\n${JSON.stringify(updateInfo, null, 2)}`);
console.log(`\n📝 Output: ${outputPath}`);
console.log(`\n💡 下一步：把 ${setupName}、${setupName}.sig、latest.json 上传到 Release ${releaseTag}`);
