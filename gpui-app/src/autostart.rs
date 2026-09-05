//! 开机自启动：HKCU\Software\Microsoft\Windows\CurrentVersion\Run 注册表项
//! 不依赖 gpui，纯注册表读写 + 独立测试子键，可单测。

#[cfg(windows)]
pub(crate) mod imp {
    use anyhow::{Context, Result};
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ,
    };

    /// 生产环境 Run 键路径（相对于 HKCU）
    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    /// 注册表值名（任务管理器"启动"页签显示名）
    const VALUE_NAME: &str = "iLauncher";
    /// 测试专用子键（绝不碰真实 Run 项）
    #[cfg(test)]
    const TEST_KEY: &str = r"Software\iLauncher_gpui_autostart_test";

    /// 当前是否已设置开机自启
    pub fn is_enabled() -> bool {
        read_value(RUN_KEY, VALUE_NAME).is_some()
    }

    /// 设置开机自启：写入带引号的当前 exe 路径
    pub fn enable() -> Result<()> {
        let exe = std::env::current_exe().context("获取当前 exe 路径失败")?;
        let cmd = format!("\"{}\"", exe.display());
        write_value(RUN_KEY, VALUE_NAME, &cmd)
    }

    /// 取消开机自启（值不存在时视为成功）
    pub fn disable() -> Result<()> {
        delete_value(RUN_KEY, VALUE_NAME)
    }

    // ── 底层原语：对任意（子键, 值名）操作，测试用独立子键隔离 ──────────────
    pub(crate) fn read_value(key_path: &str, value_name: &str) -> Option<String> {
        use std::os::windows::ffi::OsStrExt;
        let name_wide: Vec<u16> =
            std::ffi::OsStr::new(value_name).encode_wide().chain(std::iter::once(0)).collect();
        let mut buf = vec![0u16; 4096];
        let mut bytes = (buf.len() * 2) as u32;
        let key_wide = key_path_wide(key_path);
        // SAFETY：buf/key_wide/name_wide 生命周期覆盖调用，name_wide 以 NUL 结尾
        let st = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                windows::core::PCWSTR(key_wide.as_ptr()),
                windows::core::PCWSTR(name_wide.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr() as *mut _),
                Some(&mut bytes),
            )
        };
        if st.is_err() {
            return None;
        }
        // 去掉末尾 NUL，按 UTF-16 解码
        let len = (bytes as usize / 2).saturating_sub(1);
        String::from_utf16(&buf[..len]).ok()
    }

    pub(crate) fn write_value(key_path: &str, value_name: &str, data: &str) -> Result<()> {
        use std::os::windows::ffi::OsStrExt;
        let name_wide: Vec<u16> =
            std::ffi::OsStr::new(value_name).encode_wide().chain(std::iter::once(0)).collect();
        let data_wide: Vec<u16> =
            std::ffi::OsStr::new(data).encode_wide().chain(std::iter::once(0)).collect();
        let key_wide = key_path_wide(key_path);

        let mut hkey = HKEY::default();
        // SAFETY：key_wide 以 NUL 结尾，hkey 由调用填充。
        // 用 RegCreateKeyExW（打开或创建）：生产 Run 键必存在，测试子键首次写入时自动创建
        let st = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                windows::core::PCWSTR(key_wide.as_ptr()),
                0,
                None,
                Default::default(),
                KEY_SET_VALUE,
                None,
                &mut hkey,
                None,
            )
        };
        if st.is_err() {
            anyhow::bail!("打开注册表键 {} 失败，错误码 {:#X}", key_path, st.0);
        }
        // SAFETY：hkey 有效；data_wide/name_wide 以 NUL 结尾且生命周期覆盖调用
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(data_wide.as_ptr() as *const u8, data_wide.len() * 2)
        };
        let st = unsafe {
            RegSetValueExW(
                hkey,
                windows::core::PCWSTR(name_wide.as_ptr()),
                0,
                REG_SZ,
                Some(bytes),
            )
        };
        let _ = unsafe { RegCloseKey(hkey) };
        if st.is_err() {
            anyhow::bail!("写入注册表值 {} 失败，错误码 {:#X}", value_name, st.0);
        }
        Ok(())
    }

    pub(crate) fn delete_value(key_path: &str, value_name: &str) -> Result<()> {
        use std::os::windows::ffi::OsStrExt;
        let name_wide: Vec<u16> =
            std::ffi::OsStr::new(value_name).encode_wide().chain(std::iter::once(0)).collect();
        let key_wide = key_path_wide(key_path);

        let mut hkey = HKEY::default();
        // 键不存在时按成功处理（本来就未启用）
        let st = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                windows::core::PCWSTR(key_wide.as_ptr()),
                0,
                KEY_SET_VALUE,
                &mut hkey,
            )
        };
        if st.is_err() {
            return Ok(());
        }
        // SAFETY：hkey 有效；name_wide 以 NUL 结尾
        let st = unsafe { RegDeleteValueW(hkey, windows::core::PCWSTR(name_wide.as_ptr())) };
        let _ = unsafe { RegCloseKey(hkey) };
        // ERROR_FILE_NOT_FOUND(2) = 值不存在，视为成功
        if st.is_err() && st != ERROR_FILE_NOT_FOUND {
            anyhow::bail!("删除注册表值 {} 失败，错误码 {:#X}", value_name, st.0);
        }
        Ok(())
    }

    fn key_path_wide(key_path: &str) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        std::ffi::OsStr::new(key_path).encode_wide().chain(std::iter::once(0)).collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// 测试前后都清场。每个测试用独立值名——cargo test 默认多线程并行，
        /// 共享同一值名会互相踩（read_missing 会被并发 write 的测试干扰）
        fn cleanup(name: &str) {
            let _ = delete_value(TEST_KEY, name);
        }

        #[test]
        fn write_then_read_roundtrip() {
            let name = "roundtrip";
            cleanup(name);
            write_value(TEST_KEY, name, r#""C:\Program Files\iLauncher\app.exe""#).unwrap();
            let v = read_value(TEST_KEY, name).expect("应能读回");
            assert_eq!(v, r#""C:\Program Files\iLauncher\app.exe""#);
            cleanup(name);
        }

        #[test]
        fn read_missing_value_returns_none() {
            let name = "read_missing";
            cleanup(name);
            assert!(read_value(TEST_KEY, name).is_none());
            cleanup(name);
        }

        #[test]
        fn delete_missing_value_is_ok() {
            // 值不存在（甚至键都不存在）不应报错
            delete_value(TEST_KEY, "delete_missing").unwrap();
        }

        #[test]
        fn delete_removes_value() {
            let name = "delete_removes";
            cleanup(name);
            write_value(TEST_KEY, name, "\"x.exe\"").unwrap();
            delete_value(TEST_KEY, name).unwrap();
            assert!(read_value(TEST_KEY, name).is_none());
            cleanup(name);
        }

        #[test]
        fn overwrite_existing_value() {
            let name = "overwrite";
            cleanup(name);
            write_value(TEST_KEY, name, "\"v1.exe\"").unwrap();
            write_value(TEST_KEY, name, "\"v2.exe\"").unwrap();
            assert_eq!(read_value(TEST_KEY, name).as_deref(), Some("\"v2.exe\""));
            cleanup(name);
        }

        #[test]
        fn unicode_path_roundtrip() {
            let name = "unicode";
            cleanup(name);
            let cmd = r#""C:\工具\启动器\ilauncher.exe" --minimized"#;
            write_value(TEST_KEY, name, cmd).unwrap();
            assert_eq!(read_value(TEST_KEY, name).as_deref(), Some(cmd));
            cleanup(name);
        }
    }
}

#[cfg(windows)]
pub use imp::{disable, enable, is_enabled};
