// 文件预览模块

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

const MAX_PREVIEW_SIZE: u64 = 1024 * 1024; // 1MB

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePreview {
    pub content: String,
    pub file_type: FileType,
    pub size: u64,
    pub modified: String,
    pub extension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Text,
    Image,
    Markdown,
    Json,
    Code,
    Binary,
}

pub async fn read_file_preview(path: &str) -> Result<FilePreview> {
    let path = Path::new(path);
    
    if !path.exists() {
        anyhow::bail!("File does not exist");
    }
    
    if !path.is_file() {
        anyhow::bail!("Not a file");
    }
    
    let metadata = fs::metadata(path).await?;
    let size = metadata.len();
    
    let modified = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let modified = chrono::DateTime::from_timestamp(modified as i64, 0)
        .unwrap_or_default()
        .to_rfc3339();
    
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    
    let file_type = get_file_type(&extension);
    
    // 如果文件太大，返回错误
    if size > MAX_PREVIEW_SIZE {
        anyhow::bail!("File too large (max 1MB)");
    }
    
    // 对于二进制文件，不读取内容
    let content = if matches!(file_type, FileType::Binary | FileType::Image) {
        String::new()
    } else {
        match fs::read_to_string(path).await {
            Ok(content) => content,
            Err(_) => {
                // 如果无法读取为文本，标记为二进制
                return Ok(FilePreview {
                    content: String::new(),
                    file_type: FileType::Binary,
                    size,
                    modified,
                    extension,
                });
            }
        }
    };
    
    Ok(FilePreview {
        content,
        file_type,
        size,
        modified,
        extension,
    })
}

fn get_file_type(extension: &str) -> FileType {
    let ext = extension.to_lowercase();
    
    // 图片
    if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico") {
        return FileType::Image;
    }
    
    // Markdown
    if matches!(ext.as_str(), "md" | "markdown") {
        return FileType::Markdown;
    }
    
    // JSON
    if ext == "json" {
        return FileType::Json;
    }
    
    // 代码文件
    if matches!(
        ext.as_str(),
        "rs" | "js" | "jsx" | "ts" | "tsx" | "py" | "go" | "java" | "cpp" | "c" | "cs" | "php" | "rb" | "sh" | "bash" | "zsh"
        | "yml" | "yaml" | "toml" | "xml" | "html" | "css" | "scss" | "sass" | "less" | "sql" | "r" | "swift" | "kt" | "scala"
    ) {
        return FileType::Code;
    }
    
    // 纯文本
    if matches!(
        ext.as_str(),
        "txt" | "log" | "csv" | "ini" | "cfg" | "conf" | "properties" | "env" | "gitignore" | "dockerfile"
    ) {
        return FileType::Text;
    }
    
    // 默认为二进制
    FileType::Binary
}
