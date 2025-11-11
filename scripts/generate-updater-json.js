#!/usr/bin/env node

/**
 * 生成 latest.json 用于 Tauri Updater
 * 用法：node scripts/generate-updater-json.js <version> <release-tag>
 * 示例：node scripts/generate-updater-json.js 0.2.0 v0.2.0
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// 从命令行参数获取版本号和 tag
const version = process.argv[2] || process.env.VERSION || '0.1.0';
const releaseTag = process.argv[3] || process.env.RELEASE_TAG || `v${version}`;
const owner = process.env.GITHUB_OWNER || 'dulingzhi';
const repo = process.env.GITHUB_REPO || 'iLauncher';

const baseUrl = `https://github.com/${owner}/${repo}/releases/download/${releaseTag}`;

console.log(`Generating latest.json for version ${version} (tag: ${releaseTag})`);

// 查找构建产物和签名文件
const bundleDir = path.join(__dirname, '..', 'src-tauri', 'target', 'release', 'bundle');
const platforms = {};

// 读取签名文件
function readSignature(sigPath) {
  if (fs.existsSync(sigPath)) {
    return fs.readFileSync(sigPath, 'utf8').trim();
  }
  console.warn(`⚠️  Warning: Signature file not found: ${sigPath}`);
  return '';
}

// Windows (NSIS)
const windowsNsis = path.join(bundleDir, 'nsis', `iLauncher_${version}_x64-setup.nsis.zip`);
if (fs.existsSync(windowsNsis)) {
  platforms['windows-x86_64'] = {
    signature: readSignature(windowsNsis + '.sig'),
    url: `${baseUrl}/iLauncher_${version}_x64-setup.nsis.zip`
  };
  console.log('✓ Found Windows x64 NSIS installer');
}

// macOS x64
const macOSx64 = path.join(bundleDir, 'macos', `iLauncher_${version}_x64.app.tar.gz`);
if (fs.existsSync(macOSx64)) {
  platforms['darwin-x86_64'] = {
    signature: readSignature(macOSx64 + '.sig'),
    url: `${baseUrl}/iLauncher_${version}_x64.app.tar.gz`
  };
  console.log('✓ Found macOS x64 app');
}

// macOS ARM64
const macOSARM = path.join(bundleDir, 'macos', `iLauncher_${version}_aarch64.app.tar.gz`);
if (fs.existsSync(macOSARM)) {
  platforms['darwin-aarch64'] = {
    signature: readSignature(macOSARM + '.sig'),
    url: `${baseUrl}/iLauncher_${version}_aarch64.app.tar.gz`
  };
  console.log('✓ Found macOS ARM64 app');
}

// Linux AppImage
const linuxAppImage = path.join(bundleDir, 'appimage', `iLauncher_${version}_amd64.AppImage.tar.gz`);
if (fs.existsSync(linuxAppImage)) {
  platforms['linux-x86_64'] = {
    signature: readSignature(linuxAppImage + '.sig'),
    url: `${baseUrl}/iLauncher_${version}_amd64.AppImage.tar.gz`
  };
  console.log('✓ Found Linux AppImage');
}

if (Object.keys(platforms).length === 0) {
  console.error('❌ Error: No build artifacts found in src-tauri/target/release/bundle/');
  console.error('   Please run "bun tauri build" first');
  process.exit(1);
}

// 生成 latest.json
const updateInfo = {
  version: `v${version}`,
  notes: `See release notes on GitHub: https://github.com/${owner}/${repo}/releases/tag/${releaseTag}`,
  pub_date: new Date().toISOString(),
  platforms
};

const outputPath = path.join(__dirname, '..', 'latest.json');
fs.writeFileSync(outputPath, JSON.stringify(updateInfo, null, 2));

console.log('\n✅ Generated latest.json:');
console.log(JSON.stringify(updateInfo, null, 2));
console.log(`\n📝 Output: ${outputPath}`);
console.log(`\n💡 Next steps:`);
console.log(`   1. Upload latest.json to GitHub Release: ${releaseTag}`);
console.log(`   2. Upload all installer files and .sig files to the same release`);
console.log(`   3. Publish the release`);
