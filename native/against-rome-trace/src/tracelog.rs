//! One bounded UTF-8/ASCII line per write, matching the 4096-byte C++ buffer.
use std::io::{self, Write};
use std::time::Instant;

pub struct TraceLog<W: Write> {
    writer: W,
    start: Instant,
    max_bytes: Option<u64>,
    bytes_written: u64,
    stopped: bool,
}

pub fn line(elapsed: f64, thread: u32, category: &str, message: &str) -> Vec<u8> {
    bounded(format!(
        "[{elapsed:11.3}][t{thread:04x}][{category:<10}] {message}"
    ))
}

fn bounded(mut s: String) -> Vec<u8> {
    let mut end = s.len().min(4094);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
    s.push('\n');
    s.into_bytes()
}

impl<W: Write> TraceLog<W> {
    pub fn new(writer: W) -> io::Result<Self> {
        Self::with_max_bytes(writer, None)
    }

    pub fn with_max_bytes(mut writer: W, max_bytes: Option<u64>) -> io::Result<Self> {
        const BOM: &[u8] = &[0xef, 0xbb, 0xbf];
        writer.write_all(BOM)?;
        Ok(Self {
            writer,
            start: Instant::now(),
            max_bytes,
            bytes_written: BOM.len() as u64,
            stopped: false,
        })
    }

    fn write_bounded(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.stopped {
            return Ok(());
        }
        if let Some(limit) = self.max_bytes {
            if self.bytes_written.saturating_add(bytes.len() as u64) > limit {
                let warning = line(
                    self.start.elapsed().as_secs_f64() * 1000.,
                    0,
                    "warning",
                    "log size limit reached, suppressing further trace output",
                );
                let _ = self.writer.write_all(&warning);
                let _ = self.writer.flush();
                self.bytes_written = self.bytes_written.saturating_add(warning.len() as u64);
                self.stopped = true;
                return Ok(());
            }
        }
        self.writer.write_all(bytes)?;
        self.bytes_written = self.bytes_written.saturating_add(bytes.len() as u64);
        Ok(())
    }

    pub fn log(&mut self, thread: u32, category: &str, message: &str) -> io::Result<()> {
        let b = line(
            self.start.elapsed().as_secs_f64() * 1000.,
            thread,
            category,
            message,
        );
        self.write_bounded(&b)
    }

    pub fn raw(&mut self, message: &str) -> io::Result<()> {
        let b = bounded(message.to_owned());
        self.write_bounded(&b)
    }

    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// Flush, then run `sync` on the writer (C++ `LogClose` calls FlushFileBuffers
    /// before CloseHandle; `Write::flush` on a `File` alone does not do that).
    pub fn close_synced(&mut self, sync: impl FnOnce(&mut W) -> io::Result<()>) -> io::Result<()> {
        self.writer.flush()?;
        sync(&mut self.writer)
    }
    pub fn close(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
