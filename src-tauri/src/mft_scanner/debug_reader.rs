// Debug wrapper for Read trait to log all read operations

use std::io::{Read, Write, Seek, SeekFrom, Result};
use tracing::info;

pub struct DebugReader<R: Read + Seek> {
    inner: R,
    total_reads: u64,
    total_bytes: u64,
}

impl<R: Read + Seek> DebugReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            total_reads: 0,
            total_bytes: 0,
        }
    }
}

impl<R: Read + Seek> Read for DebugReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.total_reads += 1;
        let result = self.inner.read(buf);
        
        match &result {
            Ok(n) => {
                self.total_bytes += *n as u64;
                info!("   🔍 Read #{}: requested {} bytes, got {} bytes (total: {} bytes)",
                     self.total_reads, buf.len(), n, self.total_bytes);
                
                if *n < buf.len() {
                    info!("   ⚠️ Partial read! Requested {} but got {}", buf.len(), n);
                }
            }
            Err(e) => {
                info!("   ❌ Read #{} failed: {:?}", self.total_reads, e);
            }
        }
        
        result
    }
}

impl<R: Read + Seek> Seek for DebugReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        info!("   🎯 Seeking to {:?}", pos);
        let result = self.inner.seek(pos);
        if let Ok(new_pos) = result {
            info!("   ✓ Seek successful, now at position {}", new_pos);
        }
        result
    }
}

impl<R: Read + Seek> Write for DebugReader<R> {
    fn write(&mut self, _buf: &[u8]) -> Result<usize> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Write not supported"))
    }
    
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}
