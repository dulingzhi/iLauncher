// 测试 ntfs crate 是否能读取 C: 盘
// 运行: cargo run --example test_ntfs

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

fn main() {
    println!("🔧 Testing NTFS crate with C: drive...\n");
    
    // 1. 打开 C: 盘
    println!("Step 1: Opening raw disk \\\\.\\C:");
    let disk_path = "\\\\.\\C:";
    let disk_wide: Vec<u16> = disk_path.encode_utf16().chain(std::iter::once(0)).collect();
    
    use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
    use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ};
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use std::os::windows::io::FromRawHandle;
    use std::ptr::null_mut;
    
    let mut file = unsafe {
        let handle = CreateFileW(
            disk_wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null_mut(),
            OPEN_EXISTING,
            0,
            null_mut(),
        );
        
        if handle == INVALID_HANDLE_VALUE {
            let error = winapi::um::errhandlingapi::GetLastError();
            eprintln!("❌ Failed to open disk: error {}", error);
            eprintln!("   Make sure to run as Administrator!");
            return;
        }
        
        println!("✓ Disk opened successfully (handle: {:?})", handle);
        File::from_raw_handle(handle as _)
    };
    
    // 2. 读取前 512 字节（boot sector）
    println!("\nStep 2: Reading boot sector (512 bytes)");
    let mut boot_sector = vec![0u8; 512];
    match file.read_exact(&mut boot_sector) {
        Ok(_) => {
            println!("✓ Read 512 bytes successfully");
            
            // 检查 NTFS 签名
            if &boot_sector[3..11] == b"NTFS    " {
                println!("✓ NTFS signature found!");
            } else {
                eprintln!("❌ Invalid signature: {:?}", &boot_sector[3..11]);
                return;
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to read boot sector: {}", e);
            return;
        }
    }
    
    // 3. Seek 回开头
    println!("\nStep 3: Seeking back to start");
    match file.seek(SeekFrom::Start(0)) {
        Ok(_) => println!("✓ Seek successful"),
        Err(e) => {
            eprintln!("❌ Seek failed: {}", e);
            return;
        }
    }
    
    // 4. 尝试用 ntfs crate 解析
    println!("\nStep 4: Parsing with ntfs crate");
    match ntfs::Ntfs::new(&mut file) {
        Ok(ntfs) => {
            println!("✅ SUCCESS! NTFS structure parsed!");
            println!("   Serial number: {:016X}", ntfs.serial_number());
            
            // 尝试读取根目录
            println!("\nStep 5: Reading root directory");
            match ntfs.root_directory(&mut file) {
                Ok(root) => println!("✓ Root directory accessed"),
                Err(e) => eprintln!("❌ Failed to read root: {}", e),
            }
        }
        Err(e) => {
            eprintln!("❌ FAILED: {:?}", e);
            eprintln!("   Error: {:#}", e);
        }
    }
}
