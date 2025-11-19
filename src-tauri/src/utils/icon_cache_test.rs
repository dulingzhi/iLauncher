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
