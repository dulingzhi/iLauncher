//! 文件预览：对齐 旧版对应实现 的行为（去掉代码高亮 HTML，GPUI 版一期
//! 展示原文；语法高亮可后续换 gpui-component highlighter）。
//!
//! 纯同步实现（≤1MB 小文件读取毫秒级），UI 侧在后台执行器跑并做防抖；
//! 本模块无 gpui 依赖，全部单测。

use std::path::Path;

use anyhow::{Context as _, Result, bail};

/// 超过 1MB 不预览（与 旧版一致）
pub const MAX_PREVIEW_SIZE: u64 = 1024 * 1024;
/// UI 展示截断行数（内容仍完整返回，截断是渲染层行为）
pub const MAX_PREVIEW_LINES: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Text,
    Image,
    Markdown,
    Json,
    Code,
    Binary,
}



#[derive(Debug, Clone)]
pub struct FilePreview {
    pub content: String,
    pub file_type: FileType,
    /// 以下字段由元信息读取（file_meta）覆盖展示，保留供调用方直接使用与测试断言
    #[allow(dead_code)]
    pub size: u64,
    /// 修改时间（Unix 秒；0 = 拿不到）
    #[allow(dead_code)]
    pub modified_unix: u64,
    #[allow(dead_code)]
    pub extension: String,
}

pub fn get_file_type(extension: &str) -> FileType {
    let ext = extension.to_lowercase();

    // 图片（gpui img 支持的解码格式 + svg）
    if matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico"
    ) {
        return FileType::Image;
    }
    if matches!(ext.as_str(), "md" | "markdown") {
        return FileType::Markdown;
    }
    if ext == "json" {
        return FileType::Json;
    }
    // 代码文件（与 旧版同清单）
    if matches!(
        ext.as_str(),
        "rs" | "js"
            | "jsx"
            | "ts"
            | "tsx"
            | "py"
            | "go"
            | "java"
            | "cpp"
            | "c"
            | "cs"
            | "php"
            | "rb"
            | "sh"
            | "bash"
            | "zsh"
            | "yml"
            | "yaml"
            | "toml"
            | "xml"
            | "html"
            | "css"
            | "scss"
            | "sass"
            | "less"
            | "sql"
            | "r"
            | "swift"
            | "kt"
            | "scala"
    ) {
        return FileType::Code;
    }
    if matches!(
        ext.as_str(),
        "txt" | "log"
            | "csv"
            | "ini"
            | "cfg"
            | "conf"
            | "properties"
            | "env"
            | "gitignore"
            | "dockerfile"
    ) {
        return FileType::Text;
    }
    FileType::Binary
}

/// 读取预览：不存在/非文件/超大报错；二进制与图片不读内容；
/// 文本读取失败（非法 UTF-8）降级为 Binary
pub fn read_file_preview(path: &Path) -> Result<FilePreview> {
    let meta = std::fs::metadata(path).with_context(|| format!("无法访问 {}", path.display()))?;
    if !meta.is_file() {
        bail!("不是文件（可能是目录）");
    }
    let size = meta.len();
    if size > MAX_PREVIEW_SIZE {
        bail!("文件过大（上限 1MB）");
    }
    let modified_unix = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let file_type = get_file_type(&extension);

    let content = if matches!(file_type, FileType::Binary | FileType::Image) {
        String::new()
    } else {
        match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => {
                return Ok(FilePreview {
                    content: String::new(),
                    file_type: FileType::Binary,
                    size,
                    modified_unix,
                    extension,
                })
            }
        }
    };

    Ok(FilePreview {
        content,
        file_type,
        size,
        modified_unix,
        extension,
    })
}

/// 取前 N 行用于渲染（内容可能含 \r\n，按行切保留原样）
pub fn head_lines(content: &str, n: usize) -> &str {
    if content.lines().count() <= n {
        return content;
    }
    match content.match_indices('\n').nth(n - 1) {
        Some((ix, _)) => &content[..ix + 1],
        None => content,
    }
}

/// 人类可读大小：B / KB / MB（二进制单位，与 Windows 资源管理器口径一致）
pub fn human_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    if size >= MB {
        format!("{:.1} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.1} KB", size as f64 / KB as f64)
    } else {
        format!("{size} B")
    }
}

/// 文件元信息（不读内容；>1MB 与二进制文件也可获取，供预览窗口信息区用）
#[derive(Debug, Clone, Copy)]
pub struct FileMeta {
    pub size: u64,
    pub created_unix: u64,
    pub modified_unix: u64,
}

pub fn file_meta(path: &Path) -> Result<FileMeta> {
    let meta = std::fs::metadata(path).with_context(|| format!("无法访问 {}", path.display()))?;
    if !meta.is_file() {
        bail!("不是文件（可能是目录）");
    }
    let to_unix = |t: std::io::Result<std::time::SystemTime>| {
        t.ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0)
    };
    Ok(FileMeta {
        size: meta.len(),
        created_unix: to_unix(meta.created()),
        modified_unix: to_unix(meta.modified()),
    })
}

/// 相对时间（中文口径）：刚刚 / N 分钟前 / N 小时前 / N 天前 / YYYY/M/D
/// now 传当前 Unix 秒（SystemTime，与时区无关）
pub fn human_time(unix_secs: u64, now: u64) -> String {
    if unix_secs == 0 {
        return "—".to_string();
    }
    let diff = now.saturating_sub(unix_secs);
    if diff < 60 {
        "刚刚".to_string()
    } else if diff < 3600 {
        format!("{} 分钟前", diff / 60)
    } else if diff < 86400 {
        format!("{} 小时前", diff / 3600)
    } else if diff < 86400 * 7 {
        format!("{} 天前", diff / 86400)
    } else {
        format_date(unix_secs)
    }
}

/// Unix 秒 → "YYYY/M/D"（UTC，无时区依赖；展示粒度到天，时区偏移最多移动一天边界）
pub fn format_date(secs: u64) -> String {
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    format!("{y}/{m}/{d}")
}

/// 当前 Unix 秒（SystemTime::now 的 epoch 秒）
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 时间行展示："N 小时前 - 2026/9/5"（相对 + 绝对，与 Listary 预览一致）
pub fn display_time(unix_secs: u64) -> String {
    format!("{} - {}", human_time(unix_secs, now_unix()), format_date(unix_secs))
}

/// Unix 秒 → "YYYY-MM-DD HH:MM:SS"（UTC，无时区依赖）
pub fn format_unix_utc(secs: u64) -> String {
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days as i64);
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02}")
}

/// Howard Hinnant 的 days-from-civil 逆算法（纯算术，1970-01-01 = day 0）
fn civil_from_days(z: i64) -> (i64, u64, u64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(tag: &str, name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ilauncher_preview_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir.join(format!("{tag}_{name}"))
    }

    #[test]
    fn file_type_mapping_matches_legacy_version() {
        assert_eq!(get_file_type("png"), FileType::Image);
        assert_eq!(get_file_type("JPG"), FileType::Image);
        assert_eq!(get_file_type("svg"), FileType::Image);
        assert_eq!(get_file_type("md"), FileType::Markdown);
        assert_eq!(get_file_type("markdown"), FileType::Markdown);
        assert_eq!(get_file_type("json"), FileType::Json);
        assert_eq!(get_file_type("rs"), FileType::Code);
        assert_eq!(get_file_type("tsx"), FileType::Code);
        assert_eq!(get_file_type("txt"), FileType::Text);
        assert_eq!(get_file_type("log"), FileType::Text);
        assert_eq!(get_file_type("exe"), FileType::Binary);
        assert_eq!(get_file_type(""), FileType::Binary);
        assert_eq!(get_file_type("unknownext"), FileType::Binary);
    }

    #[test]
    fn read_text_file_roundtrip() {
        let path = temp_file("txt", "a.txt");
        std::fs::write(&path, "hello\n世界\n").unwrap();
        let p = read_file_preview(&path).unwrap();
        assert_eq!(p.content, "hello\n世界\n");
        assert_eq!(p.file_type, FileType::Text);
        assert_eq!(p.size, 13); // hello\n=6 + 世界=6 + \n=1
        assert_eq!(p.extension, "txt");
        assert!(p.modified_unix > 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn image_type_skips_content_read() {
        // 内容不是合法图片也没关系——按扩展名判定，不读内容
        let path = temp_file("img", "a.png");
        std::fs::write(&path, b"not-a-real-png").unwrap();
        let p = read_file_preview(&path).unwrap();
        assert_eq!(p.file_type, FileType::Image);
        assert!(p.content.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_utf8_text_downgrades_to_binary() {
        let path = temp_file("bin", "a.txt");
        std::fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();
        let p = read_file_preview(&path).unwrap();
        assert_eq!(p.file_type, FileType::Binary);
        assert!(p.content.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn oversized_file_rejected() {
        let path = temp_file("big", "a.txt");
        std::fs::write(&path, vec![0u8; (MAX_PREVIEW_SIZE + 1) as usize]).unwrap();
        assert!(read_file_preview(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn directory_and_missing_rejected() {
        let dir = temp_file("dir", "");
        let _ = std::fs::create_dir_all(&dir);
        assert!(read_file_preview(&dir).is_err());
        assert!(read_file_preview(&dir.join("nope.txt")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn head_lines_truncates() {
        let content = "a\nb\nc\nd\ne\n";
        assert_eq!(head_lines(content, 3), "a\nb\nc\n");
        assert_eq!(head_lines(content, 100), content);
        assert_eq!(head_lines("", 3), "");
        // 不足 N 行不截断
        assert_eq!(head_lines("a\nb", 3), "a\nb");
    }

    #[test]
    fn human_size_formats() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn file_meta_reads_without_content() {
        let path = temp_file("meta", "a.bin");
        std::fs::write(&path, [0u8; 42]).unwrap();
        let m = file_meta(&path).unwrap();
        assert_eq!(m.size, 42);
        assert!(m.created_unix > 0);
        assert!(m.modified_unix > 0);
        // 目录报错
        let dir = temp_file("metadir", "");
        let _ = std::fs::create_dir_all(&dir);
        assert!(file_meta(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn human_time_relative_buckets() {
        let now = 1_700_000_000u64;
        assert_eq!(human_time(0, now), "—");
        assert_eq!(human_time(now - 10, now), "刚刚");
        assert_eq!(human_time(now - 300, now), "5 分钟前");
        assert_eq!(human_time(now - 7200, now), "2 小时前");
        assert_eq!(human_time(now - 3 * 86400, now), "3 天前");
        // 超过 7 天退化为绝对日期（1700000000 = 2023/11/14 UTC）
        assert_eq!(human_time(now - 10 * 86400, now), "2023/11/4");
    }

    #[test]
    fn format_date_known_values() {
        assert_eq!(format_date(0), "1970/1/1");
        assert_eq!(format_date(86_400), "1970/1/2");
        assert_eq!(format_date(1_709_164_800), "2024/2/29");
    }

    #[test]
    fn format_unix_utc_known_values() {
        assert_eq!(format_unix_utc(0), "1970-01-01 00:00:00");
        assert_eq!(format_unix_utc(86_399), "1970-01-01 23:59:59");
        assert_eq!(format_unix_utc(86_400), "1970-01-02 00:00:00");
        // 2024-02-29（闰日）00:00:00 UTC = 1709164800
        assert_eq!(format_unix_utc(1_709_164_800), "2024-02-29 00:00:00");
    }
}
