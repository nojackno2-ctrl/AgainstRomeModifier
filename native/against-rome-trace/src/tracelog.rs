//! One bounded UTF-8/ASCII line per write, matching the 4096-byte C++ buffer.
use std::io::{self, Write};
use std::time::Instant;
pub struct TraceLog<W: Write> {
    writer: W,
    start: Instant,
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
    pub fn new(mut writer: W) -> io::Result<Self> {
        writer.write_all(&[0xef, 0xbb, 0xbf])?;
        Ok(Self {
            writer,
            start: Instant::now(),
        })
    }
    pub fn log(&mut self, thread: u32, category: &str, message: &str) -> io::Result<()> {
        self.writer.write_all(&line(
            self.start.elapsed().as_secs_f64() * 1000.,
            thread,
            category,
            message,
        ))
    }
    pub fn raw(&mut self, message: &str) -> io::Result<()> {
        self.writer.write_all(&bounded(message.to_owned()))
    }
    pub fn close(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
