use std::sync::atomic::{AtomicU64, Ordering};

/// 跨試行子系統的狀態序號，亦用於合併提交通知的先後順序。
pub fn next_history_token() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Debug)]
pub struct HistoryTokens {
    current: u64,
    undo: Vec<(u64, u64)>,
    redo: Vec<(u64, u64)>,
    receipts: Vec<(u64, u64)>,
}
impl Default for HistoryTokens {
    fn default() -> Self {
        Self { current: next_history_token(), undo: vec![], redo: vec![], receipts: vec![] }
    }
}
impl HistoryTokens {
    pub fn token(&self) -> u64 { self.current }
    pub fn commit(&mut self) {
        let step = (self.current, next_history_token());
        self.current = step.1;
        self.undo.push(step);
        self.redo.clear();
        self.receipts.push(step);
    }
    pub fn undo(&mut self) {
        if let Some(step) = self.undo.pop() { self.current = step.0; self.redo.push(step); }
    }
    pub fn redo(&mut self) {
        if let Some(step) = self.redo.pop() { self.current = step.1; self.undo.push(step); }
    }
    pub fn clear_redo(&mut self) { self.redo.clear(); }
    pub fn take_receipts(&mut self) -> Vec<(u64, u64)> { std::mem::take(&mut self.receipts) }
}
