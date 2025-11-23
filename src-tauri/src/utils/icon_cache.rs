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
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{DeleteObject, CreateCompatibleDC, SelectObject, BI_RGB, DIB_RGB_COLORS};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;

/// 图标缓存（扩展名 -> base64）
#[cfg(target_os = "windows")]
static ICON_CACHE: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// 获取文件图标的 base64 编码
#[cfg(target_os = "windows")]
pub fn get_file_icon_base64(file_path: &str, is_dir: bool) -> Result<String> {
    // 生成缓存键
    let cache_key = if is_dir {
        "__folder__".to_string()
    } else {
        let ext = Path::new(file_path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        // 🔥 .exe/.ico/.dll 等包含自定义图标的文件，按完整路径缓存
        // 其他文件类型（.txt/.pdf/.docx 等）按扩展名缓存
        if ext == "exe" || ext == "ico" || ext == "dll" || ext == "lnk" {
            file_path.to_lowercase()
        } else if ext.is_empty() {
            "__no_ext__".to_string()
        } else {
            format!(".{}", ext)
        }
    };
    
    // 检查内存缓存
    if let Ok(cache) = ICON_CACHE.lock() {
        if let Some(cached_base64) = cache.get(&cache_key) {
            return Ok(cached_base64.clone());
        }
    }
    
    // 缓存未命中，提取图标
    let base64_data = extract_icon_as_base64(file_path)?;
    
    // 更新内存缓存
    if let Ok(mut cache) = ICON_CACHE.lock() {
        cache.insert(cache_key, base64_data.clone());
    }
    
    Ok(base64_data)
}

/// 提取图标并转换为 base64
#[cfg(target_os = "windows")]
fn extract_icon_as_base64(file_path: &str) -> Result<String> {
    use std::os::windows::ffi::OsStrExt;
    
    // 转换为 UTF-16
    let wide_path: Vec<u16> = std::ffi::OsStr::new(file_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    
    unsafe {
        // 🔥 使用 SHGFI_LARGEICON 获取 32x32 图标（系统标准大图标）
        // 注意：Windows 系统图标有固定尺寸，我们在后续绘制时放大到 48x48
        let mut shfi: SHFILEINFOW = std::mem::zeroed();
        let result = SHGetFileInfoW(
            PCWSTR(wide_path.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut shfi),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,  // 获取大图标 (32x32)
        );
        
        if result == 0 || shfi.hIcon.is_invalid() {
            return Err(anyhow::anyhow!("Failed to get file icon"));
        }
        
        let hicon = shfi.hIcon;
        
        // 将图标转换为 PNG base64
        let base64_data = icon_to_base64(hicon)?;
        
        // 释放图标
        let _ = DestroyIcon(hicon);
        
        Ok(format!("data:image/png;base64,{}", base64_data))
    }
}

/// 将 HICON 转换为 base64 编码的 PNG
#[cfg(target_os = "windows")]
fn icon_to_base64(hicon: windows::Win32::UI::WindowsAndMessaging::HICON) -> Result<String> {
    use image::{ImageBuffer, ImageEncoder, Rgba};
    use windows::Win32::Graphics::Gdi::{CreateDIBSection, BITMAPINFO, BITMAPINFOHEADER};
    
    unsafe {
        // 🔥 创建一个 48x48 的位图来绘制图标（提升清晰度）
        let icon_size: u32 = 48;
        
        // 创建设备上下文
        let hdc = CreateCompatibleDC(None);
        if hdc.is_invalid() {
            return Err(anyhow::anyhow!("Failed to create DC"));
        }
        
        // 准备 BITMAPINFO
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = icon_size as i32;
        bmi.bmiHeader.biHeight = -(icon_size as i32); // 负数表示自顶向下
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;
        
        // 创建 DIB Section
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let hbitmap = CreateDIBSection(
            hdc,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )?;
        
        if hbitmap.is_invalid() || bits.is_null() {
            let _ = windows::Win32::Graphics::Gdi::DeleteDC(hdc);
            return Err(anyhow::anyhow!("Failed to create DIB section"));
        }
        
        // 选择位图到 DC
        let old_bitmap = SelectObject(hdc, hbitmap);
        
        // 🔥 将图标绘制到位图上（这会自动缩放到 32x32）
        let draw_result = windows::Win32::UI::WindowsAndMessaging::DrawIconEx(
            hdc,
            0,
            0,
            hicon,
            icon_size as i32,
            icon_size as i32,
            0,
            None,
            windows::Win32::UI::WindowsAndMessaging::DI_NORMAL,
        );
        
        if draw_result.is_err() {
            SelectObject(hdc, old_bitmap);
            let _ = DeleteObject(hbitmap);
            let _ = windows::Win32::Graphics::Gdi::DeleteDC(hdc);
            return Err(anyhow::anyhow!("Failed to draw icon"));
        }
        
        // 从 bits 指针读取像素数据
        let buffer_size = (icon_size * icon_size * 4) as usize;
        let pixels = std::slice::from_raw_parts(bits as *const u8, buffer_size).to_vec();
        
        // 清理
        SelectObject(hdc, old_bitmap);
        let _ = DeleteObject(hbitmap);
        let _ = windows::Win32::Graphics::Gdi::DeleteDC(hdc);
        
        // 转换为 RGBA 格式（Windows 是 BGRA）
        let img_buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_fn(icon_size, icon_size, |x, y| {
            let idx = ((y * icon_size + x) * 4) as usize;
            Rgba([
                pixels[idx + 2], // B -> R
                pixels[idx + 1], // G
                pixels[idx],     // R -> B
                pixels[idx + 3], // A
            ])
        });
        
        // 转换为 RGBA 格式（Windows 是 BGRA）
        let img_buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_fn(icon_size, icon_size, |x, y| {
            let idx = ((y * icon_size + x) * 4) as usize;
            Rgba([
                pixels[idx + 2], // B -> R
                pixels[idx + 1], // G
                pixels[idx],     // R -> B
                pixels[idx + 3], // A
            ])
        });
        
        // 🔥 将图片编码为 PNG 并转换为 base64
        let mut png_data = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
        encoder.write_image(
            &img_buffer,
            icon_size,
            icon_size,
            image::ExtendedColorType::Rgba8,
        )?;
        
        let base64_data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png_data);
        
        Ok(base64_data)
    }
}

/// 清理图标缓存（清空内存缓存）
#[cfg(target_os = "windows")]
pub fn clear_icon_cache() -> Result<()> {
    if let Ok(mut cache) = ICON_CACHE.lock() {
        cache.clear();
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
            let _ = get_file_icon_base64(path, is_dir);
        }
    });
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;

    #[test]
    fn test_icon_extraction() {
        // 测试文件夹图标
        let folder_icon = get_file_icon_base64("C:\\", true);
        println!("Folder icon result: {:?}", folder_icon);
        assert!(folder_icon.is_ok(), "Failed to extract folder icon: {:?}", folder_icon.err());
        
        let icon_data = folder_icon.unwrap();
        println!("Folder icon data length: {}", icon_data.len());
        assert!(icon_data.starts_with("data:image/png;base64,"), "Invalid base64 format");
        
        // 测试文件图标  
        let file_icon = get_file_icon_base64("C:\\Windows\\notepad.exe", false);
        println!("File icon result: {:?}", file_icon);
        assert!(file_icon.is_ok(), "Failed to extract file icon: {:?}", file_icon.err());
        
        let icon_data = file_icon.unwrap();
        println!("File icon data length: {}", icon_data.len());
        assert!(icon_data.starts_with("data:image/png;base64,"), "Invalid base64 format");
    }
}
