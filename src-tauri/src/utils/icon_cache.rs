// Windows 文件图标提取和缓存

#[cfg(target_os = "windows")]
use anyhow::Result;
#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::path::Path;
#[cfg(target_os = "windows")]
use std::sync::Mutex;
#[cfg(target_os = "windows")]
use once_cell::sync::Lazy;

#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{DeleteObject, GetDIBits, CreateCompatibleDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;

/// 图标缓存（扩展名 -> 图标文件路径）
#[cfg(target_os = "windows")]
static ICON_CACHE: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// 获取文件图标路径（带缓存，快速返回）
#[cfg(target_os = "windows")]
pub fn get_file_icon(file_path: &str, is_dir: bool) -> Result<String> {
    tracing::debug!("📦 icon_cache::get_file_icon called: {} (is_dir: {})", file_path, is_dir);
    
    // 1. 对于目录，使用统一的文件夹图标
    if is_dir {
        let cache_key = "__folder__".to_string();
        
        if let Ok(cache) = ICON_CACHE.lock() {
            if let Some(cached_path) = cache.get(&cache_key) {
                if Path::new(cached_path).exists() {
                    tracing::debug!("✓ Using cached folder icon: {}", cached_path);
                    return Ok(cached_path.clone());
                }
            }
        }
        
        tracing::debug!("⚡ Extracting folder icon...");
        // 提取文件夹图标（使用通用路径）
        let icon_path = extract_icon_to_temp("C:\\", true)?;
        
        if let Ok(mut cache) = ICON_CACHE.lock() {
            cache.insert(cache_key, icon_path.clone());
        }
        
        tracing::debug!("✓ Folder icon extracted: {}", icon_path);
        return Ok(icon_path);
    }
    
    // 2. 对于文件，按扩展名缓存
    let ext = Path::new(file_path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    let cache_key = if ext.is_empty() {
        "__no_ext__".to_string()
    } else {
        format!(".{}", ext)
    };
    
    // 检查缓存
    if let Ok(cache) = ICON_CACHE.lock() {
        if let Some(cached_path) = cache.get(&cache_key) {
            if Path::new(cached_path).exists() {
                return Ok(cached_path.clone());
            }
        }
    }
    
    // 缓存未命中，提取图标
    let icon_path = extract_icon_to_temp(file_path, false)?;
    
    // 更新缓存
    if let Ok(mut cache) = ICON_CACHE.lock() {
        cache.insert(cache_key, icon_path.clone());
    }
    
    Ok(icon_path)
}

/// 提取图标到临时文件
#[cfg(target_os = "windows")]
fn extract_icon_to_temp(file_path: &str, _is_dir: bool) -> Result<String> {
    use std::os::windows::ffi::OsStrExt;
    
    // 转换为 UTF-16
    let wide_path: Vec<u16> = std::ffi::OsStr::new(file_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    
    unsafe {
        let mut shfi: SHFILEINFOW = std::mem::zeroed();
        
        // 获取小图标 (16x16)
        let result = SHGetFileInfoW(
            PCWSTR(wide_path.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut shfi),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_SMALLICON,
        );
        
        if result == 0 || shfi.hIcon.is_invalid() {
            return Err(anyhow::anyhow!("Failed to get file icon"));
        }
        
        // 将图标保存为 PNG
        let icon_path = save_icon_as_png(shfi.hIcon)?;
        
        // 释放图标
        let _ = DestroyIcon(shfi.hIcon);
        
        Ok(icon_path)
    }
}

/// 将 HICON 保存为 PNG 文件
#[cfg(target_os = "windows")]
fn save_icon_as_png(hicon: windows::Win32::UI::WindowsAndMessaging::HICON) -> Result<String> {
    use image::{ImageBuffer, Rgba};
    
    unsafe {
        // 获取图标信息
        let mut icon_info: ICONINFO = std::mem::zeroed();
        if GetIconInfo(hicon, &mut icon_info).is_err() {
            return Err(anyhow::anyhow!("Failed to get icon info"));
        }
        
        // 创建设备上下文
        let hdc = CreateCompatibleDC(None);
        if hdc.is_invalid() {
            DeleteObject(icon_info.hbmColor);
            DeleteObject(icon_info.hbmMask);
            return Err(anyhow::anyhow!("Failed to create DC"));
        }
        
        // 准备 BITMAPINFO
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = 16;
        bmi.bmiHeader.biHeight = -16; // 负数表示自顶向下
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;
        
        // 分配像素缓冲区
        let mut pixels = vec![0u8; 16 * 16 * 4];
        
        // 选择位图到 DC
        let old_bitmap = SelectObject(hdc, icon_info.hbmColor);
        
        // 读取像素数据
        let result = GetDIBits(
            hdc,
            icon_info.hbmColor,
            0,
            16,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        
        // 恢复并清理
        SelectObject(hdc, old_bitmap);
        let _ = windows::Win32::Graphics::Gdi::DeleteDC(hdc);
        let _ = DeleteObject(icon_info.hbmColor);
        let _ = DeleteObject(icon_info.hbmMask);
        
        if result == 0 {
            return Err(anyhow::anyhow!("Failed to get bitmap bits"));
        }
        
        // 转换为 RGBA 格式（Windows 是 BGRA）
        let img_buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_fn(16, 16, |x, y| {
            let idx = ((y * 16 + x) * 4) as usize;
            Rgba([
                pixels[idx + 2], // B -> R
                pixels[idx + 1], // G
                pixels[idx],     // R -> B
                pixels[idx + 3], // A
            ])
        });
        
        // 保存到临时目录
        let temp_dir = std::env::temp_dir().join("ilauncher_icons");
        std::fs::create_dir_all(&temp_dir)?;
        
        let icon_path = temp_dir.join(format!("icon_{}.png", uuid::Uuid::new_v4()));
        img_buffer.save(&icon_path)?;
        
        Ok(icon_path.to_string_lossy().to_string())
    }
}

/// 清理图标缓存
#[cfg(target_os = "windows")]
pub fn clear_icon_cache() -> Result<()> {
    if let Ok(mut cache) = ICON_CACHE.lock() {
        // 删除所有缓存的图标文件
        for (_, path) in cache.iter() {
            let _ = std::fs::remove_file(path);
        }
        cache.clear();
    }
    
    // 清理临时目录
    let temp_dir = std::env::temp_dir().join("ilauncher_icons");
    if temp_dir.exists() {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    
    Ok(())
}

/// 预热常见文件类型的图标缓存
#[cfg(target_os = "windows")]
pub fn warmup_icon_cache() {
    use std::thread;
    
    thread::spawn(|| {
        let common_extensions = vec![
            ("test.txt", false),
            ("test.pdf", false),
            ("test.doc", false),
            ("test.xls", false),
            ("test.jpg", false),
            ("test.png", false),
            ("test.mp3", false),
            ("test.mp4", false),
            ("test.zip", false),
            ("test.exe", false),
            ("C:\\", true), // 文件夹
        ];
        
        for (path, is_dir) in common_extensions {
            let _ = get_file_icon(path, is_dir);
        }
        
        tracing::info!("✓ Icon cache warmed up");
    });
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;

    #[test]
    fn test_icon_extraction() {
        // 测试文件夹图标
        let folder_icon = get_file_icon("C:\\", true);
        println!("Folder icon result: {:?}", folder_icon);
        assert!(folder_icon.is_ok(), "Failed to extract folder icon: {:?}", folder_icon.err());
        
        let icon_path = folder_icon.unwrap();
        println!("Folder icon path: {}", icon_path);
        assert!(std::path::Path::new(&icon_path).exists(), "Icon file doesn't exist");
        
        // 测试文件图标  
        let file_icon = get_file_icon("C:\\Windows\\notepad.exe", false);
        println!("File icon result: {:?}", file_icon);
        assert!(file_icon.is_ok(), "Failed to extract file icon: {:?}", file_icon.err());
        
        let icon_path = file_icon.unwrap();
        println!("File icon path: {}", icon_path);
        assert!(std::path::Path::new(&icon_path).exists(), "Icon file doesn't exist");
    }
}
