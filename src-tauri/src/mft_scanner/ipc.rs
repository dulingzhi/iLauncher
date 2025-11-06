// IPC 通信模块 - 使用TCP连接 (绕过Windows命名管道权限问题)

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::io::{BufReader, BufWriter, Read, Write};
use std::net::{TcpListener, TcpStream};
use tracing::info;

use super::scanner::MftFileEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcMessage {
    // 客户端 -> 服务端
    ScanDrive { drive_letter: char },
    Ping,
    Shutdown,
    
    // 服务端 -> 客户端
    ScanProgress { files_count: usize },
    ScanComplete { entries: Vec<MftFileEntry> },
    ScanError { error: String },
    Pong,
}

const IPC_PORT: u16 = 34567;

/// MFT 扫描器服务端（运行在管理员进程）
pub struct ScannerServer;

impl ScannerServer {
    pub fn run() -> Result<()> {
        info!("🔧 Starting MFT Scanner Server on TCP port {}...", IPC_PORT);
        
        let listener = TcpListener::bind(format!("127.0.0.1:{}", IPC_PORT))?;
        
        info!("✅ Server listening on 127.0.0.1:{}", IPC_PORT);
        
        for stream in listener.incoming() {
            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("❌ Connection error: {}", e);
                    continue;
                }
            };
            
            info!("📞 Client connected");
            
            if let Err(e) = Self::handle_client(stream) {
                eprintln!("❌ Client error: {}", e);
            }
        }
        
        Ok(())
    }
    
    fn handle_client(stream: TcpStream) -> Result<()> {
        let mut reader = BufReader::new(&stream);
        let mut writer = BufWriter::new(&stream);
        
        loop {
            // 读取消息长度（4字节）
            let mut len_buf = [0u8; 4];
            if reader.read_exact(&mut len_buf).is_err() {
                break; // 客户端断开
            }
            let msg_len = u32::from_le_bytes(len_buf) as usize;
            
            // 读取消息内容
            let mut msg_buf = vec![0u8; msg_len];
            reader.read_exact(&mut msg_buf)?;
            
            let message: IpcMessage = serde_json::from_slice(&msg_buf)?;
            
            match message {
                IpcMessage::Ping => {
                    Self::send_message(&mut writer, &IpcMessage::Pong)?;
                }
                
                IpcMessage::ScanDrive { drive_letter } => {
                    use super::scanner::UsnScanner;
                    use tracing::info;
                    
                    info!("🔍 Received scan request for drive {}", drive_letter);
                    
                    let scanner = UsnScanner::new(drive_letter);
                    match scanner.scan() {
                        Ok(entries) => {
                            info!("✅ Scan complete: {} files", entries.len());
                            Self::send_message(&mut writer, &IpcMessage::ScanComplete { entries })?;
                        }
                        Err(e) => {
                            let error_msg = format!("{:#}", e);
                            eprintln!("❌ Scan error: {}", error_msg);
                            Self::send_message(&mut writer, &IpcMessage::ScanError { error: error_msg })?;
                        }
                    }
                }
                
                IpcMessage::Shutdown => {
                    info!("🛑 Received shutdown command");
                    break;
                }
                
                _ => {
                    eprintln!("⚠️ Unexpected message: {:?}", message);
                }
            }
        }
        
        Ok(())
    }
    
    fn send_message<W: Write>(writer: &mut W, message: &IpcMessage) -> Result<()> {
        let json = serde_json::to_vec(message)?;
        let len = (json.len() as u32).to_le_bytes();
        
        writer.write_all(&len)?;
        writer.write_all(&json)?;
        writer.flush()?;
        
        Ok(())
    }
}

/// MFT 扫描器客户端（运行在主进程）
pub struct ScannerClient {
    stream: TcpStream,
}

impl ScannerClient {
    pub fn connect() -> Result<Self> {
        use std::time::Duration;
        
        tracing::info!("Attempting to connect to MFT scanner on localhost:{}...", IPC_PORT);
        
        // 重试连接（等待服务端启动）
        for attempt in 0..15 {
            match TcpStream::connect_timeout(
                &format!("127.0.0.1:{}", IPC_PORT).parse().unwrap(),
                Duration::from_millis(200)
            ) {
                Ok(stream) => {
                    tracing::info!("✅ Connected to MFT scanner on attempt #{}", attempt + 1);
                    return Ok(Self { stream });
                }
                Err(e) if attempt < 14 => {
                    tracing::debug!("Connection attempt #{} failed: {}, retrying...", attempt + 1, e);
                    std::thread::sleep(Duration::from_millis(300));
                    continue;
                }
                Err(e) => {
                    return Err(anyhow::Error::new(e))
                        .context("Failed to connect to MFT scanner after 15 attempts");
                }
            }
        }
        
        unreachable!()
    }
    
    pub fn ping(&mut self) -> Result<()> {
        self.send(&IpcMessage::Ping)?;
        
        match self.receive()? {
            IpcMessage::Pong => Ok(()),
            _ => Err(anyhow::anyhow!("Invalid response to ping")),
        }
    }
    
    pub fn scan_drive(&mut self, drive_letter: char) -> Result<Vec<MftFileEntry>> {
        self.send(&IpcMessage::ScanDrive { drive_letter })?;
        
        match self.receive()? {
            IpcMessage::ScanComplete { entries } => Ok(entries),
            IpcMessage::ScanError { error } => Err(anyhow::anyhow!("Scan error: {}", error)),
            _ => Err(anyhow::anyhow!("Invalid response to scan request")),
        }
    }
    
    pub fn shutdown(&mut self) -> Result<()> {
        self.send(&IpcMessage::Shutdown)?;
        Ok(())
    }
    
    fn send(&mut self, message: &IpcMessage) -> Result<()> {
        let json = serde_json::to_vec(message)?;
        let len = (json.len() as u32).to_le_bytes();
        
        self.stream.write_all(&len)?;
        self.stream.write_all(&json)?;
        self.stream.flush()?;
        
        Ok(())
    }
    
    fn receive(&mut self) -> Result<IpcMessage> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf)?;
        let msg_len = u32::from_le_bytes(len_buf) as usize;
        
        let mut msg_buf = vec![0u8; msg_len];
        self.stream.read_exact(&mut msg_buf)?;
        
        let message = serde_json::from_slice(&msg_buf)?;
        Ok(message)
    }
}
