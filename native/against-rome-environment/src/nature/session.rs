//! NatureEditSession: Pending nature object modifications, stroke history, undo and redo.

use super::engine::NatureAddition;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

// 獨立核心預設使用自己的序號區間；編輯器須注入 Step2 的共用配發器以排序跨工具提交。
fn next_nature_history_token() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1 << 63);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod history_tests {
    use super::*;
    use crate::nature::stroke::NatureStrokeDriver;

    fn addition(x: f32) -> NatureAddition {
        NatureAddition::new(1, "LanTree", x, 0.0, 10.0, 0.0)
    }

    #[test]
    fn cancelled_stroke_keeps_prior_receipts_and_original_redo_behavior() {
        let mut session = NatureEditSession::new();
        session.plant(addition(10.0));
        assert!(session.commit_stroke());
        let token = session.history_token();
        let before = session.capture();
        session.plant(addition(20.0));
        assert!(session.cancel_stroke());
        assert_eq!(session.history_token(), token);
        assert_eq!(session.capture(), before);
        assert_eq!(session.take_history_receipts().len(), 1);
        assert!(session.can_redo());
        assert!(!session.cancel_stroke());
        assert!(session.take_history_receipts().is_empty());
    }
    #[test]
    fn shadow_history_token_roundtrip_and_stroke_receipt() {
        let mut session = NatureEditSession::new();
        let mut driver = NatureStrokeDriver::new();
        let original = session.history_token();
        let before = session.capture();
        session.plant(addition(10.0));
        session.plant(addition(20.0));
        assert_eq!(session.history_token(), original);
        assert!(session.take_history_receipts().is_empty());
        // 與 nature_end_stroke() 相同入口，一筆多次 record() 只提交一步。
        assert!(driver.end(&mut session));
        let first = session.history_token();
        let after = session.capture();
        assert_ne!(original, first);
        assert_eq!(session.take_history_receipts(), vec![(original, first)]);
        assert!(session.undo());
        assert_eq!(session.history_token(), original);
        assert_eq!(session.capture(), before);
        assert!(session.redo());
        assert_eq!(session.history_token(), first);
        assert_eq!(session.capture(), after);
        assert!(session.take_history_receipts().is_empty());
    }

    #[test]
    fn shadow_history_noop_and_rejected_layout_emit_nothing() {
        let mut session = NatureEditSession::new();
        let original = session.history_token();
        assert!(!session.commit_stroke());
        assert!(!session.undo());
        assert!(!session.redo());
        assert!(!session.remove(&[], &[]));
        assert!(!session.plant_many(&[]).unwrap());
        assert!(session.plant_many(&[addition(10.0), addition(f32::NAN)]).is_err());
        assert!(session.plant_many(&[NatureAddition::new(1, "", 1.0, 0.0, 1.0, 0.0)]).is_err());
        assert_eq!(session.history_token(), original);
        assert!(session.take_history_receipts().is_empty());
        session.plant(addition(10.0));
        assert!(session.remove(&[], &[addition(10.0)]));
        assert!(session.commit_stroke(), "保留既有核心相同值筆觸歷史");
        assert_eq!(session.history_token(), original);
        assert!(session.undo());
        assert!(session.redo());
        assert_eq!(session.history_token(), original);
        assert!(session.take_history_receipts().is_empty());
    }

    #[test]
    fn shadow_history_branch_clears_redo_at_record_and_preserves_undo() {
        let mut session = NatureEditSession::new();
        session.plant_many(&[addition(10.0)]).unwrap();
        let first = session.history_token();
        session.plant_many(&[addition(20.0)]).unwrap();
        let abandoned = session.history_token();
        assert!(session.undo());
        assert_eq!(session.history_token(), first);
        assert!(session.can_redo());
        session.take_history_receipts();
        session.plant(addition(30.0));
        assert!(!session.can_redo());
        assert!(session.history.redo.is_empty());
        assert!(session.take_history_receipts().is_empty());
        assert!(session.commit_stroke());
        assert_ne!(session.history_token(), abandoned);
        assert_eq!(session.take_history_receipts().len(), 1);
        assert!(session.undo());
        session.clear_redo();
        assert_eq!(session.history_token(), first);
        assert!(!session.can_redo());
        assert!(session.undo());
        assert!(session.take_history_receipts().is_empty());
    }

    #[test]
    fn shadow_history_allocator_survives_load_reset_and_clone() {
        fn allocate() -> u64 {
            static NEXT: AtomicU64 = AtomicU64::new(1000);
            NEXT.fetch_add(1, Ordering::Relaxed)
        }
        let mut session = NatureEditSession::new_with_history_token_allocator(allocate);
        let original = session.history_token();
        session.plant_many(&[addition(10.0)]).unwrap();
        assert_eq!(session.take_history_receipts(), vec![(original, original + 1)]);
        session.accept_changes();
        let accepted = session.history_token();
        assert_eq!(accepted, original + 2);
        assert!(session.take_history_receipts().is_empty());
        assert!(!session.can_undo() && !session.can_redo());
        let mut clone = session.clone();
        clone.plant_many(&[addition(20.0)]).unwrap();
        assert_eq!(clone.take_history_receipts(), vec![(accepted, accepted + 1)]);
        session.reset();
        assert_eq!(session.history_token(), accepted + 2);
        assert!(session.take_history_receipts().is_empty());
    }
}

#[derive(Debug, Clone)]
struct NatureHistory {
    allocate: fn() -> u64,
    current: u64,
    undo: Vec<(u64, u64)>,
    redo: Vec<(u64, u64)>,
    receipts: Vec<(u64, u64)>,
}

impl NatureHistory {
    fn new(allocate: fn() -> u64) -> Self {
        Self { allocate, current: allocate(), undo: vec![], redo: vec![], receipts: vec![] }
    }
    fn commit(&mut self, changed: bool) {
        let before = self.current;
        if changed { self.current = (self.allocate)(); }
        let step = (before, self.current);
        self.undo.push(step);
        self.redo.clear();
        if changed { self.receipts.push(step); }
    }
    fn undo(&mut self) {
        if let Some(step) = self.undo.pop() { self.current = step.0; self.redo.push(step); }
    }
    fn redo(&mut self) {
        if let Some(step) = self.redo.pop() { self.current = step.1; self.undo.push(step); }
    }
}

/// 自然物件快照。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NatureSnapshot {
    pub removed_slots: Vec<i32>,
    pub additions: Vec<NatureAddition>,
}

#[derive(Debug, Clone, PartialEq)]
struct IndexedAddition {
    index: usize,
    item: NatureAddition,
}

#[derive(Debug, Clone, PartialEq)]
struct Operation {
    added: Option<IndexedAddition>,
    removed: Vec<i32>,
    removed_additions: Vec<IndexedAddition>,
}

/// 待存的自然物件變更與筆觸歷史。
#[derive(Debug, Clone)]
pub struct NatureEditSession {
    removals: BTreeSet<i32>,
    additions: Vec<NatureAddition>,
    undo_stack: Vec<Vec<Operation>>,
    redo_stack: Vec<Vec<Operation>>,
    stroke: Vec<Operation>,
    baseline: NatureSnapshot,
    history: NatureHistory,
    stroke_before: Option<NatureSnapshot>,
}

impl Default for NatureEditSession {
    fn default() -> Self {
        Self::new()
    }
}

impl NatureEditSession {
    pub fn new() -> Self {
        Self::new_with_history_token_allocator(next_nature_history_token)
    }

    /// 編輯器傳入 against_rome_textures::history::next_history_token，與 Step2 共用排序序號。
    pub fn new_with_history_token_allocator(allocate: fn() -> u64) -> Self {
        Self {
            removals: BTreeSet::new(),
            additions: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            stroke: Vec::new(),
            baseline: NatureSnapshot::default(),
            history: NatureHistory::new(allocate),
            stroke_before: None,
        }
    }

    /// 可在建立編輯器後、首次編輯前設定；既有 token 與歷史不變。
    pub fn set_history_token_allocator(&mut self, allocate: fn() -> u64) {
        self.history.allocate = allocate;
    }

    pub fn history_token(&self) -> u64 { self.history.current }

    pub fn take_history_receipts(&mut self) -> Vec<(u64, u64)> {
        std::mem::take(&mut self.history.receipts)
    }

    /// 保留復原、目前狀態與進行中的筆觸，只移除重做。
    pub fn clear_redo(&mut self) {
        self.redo_stack.clear();
        self.history.redo.clear();
    }

    pub fn module_id(&self) -> &'static str {
        "nature"
    }

    pub fn is_dirty(&self) -> bool {
        let current_removals: Vec<i32> = self.removals.iter().copied().collect();
        current_removals != self.baseline.removed_slots || self.additions != self.baseline.additions
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty() || !self.stroke.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn removed_slots(&self) -> &BTreeSet<i32> {
        &self.removals
    }

    pub fn additions(&self) -> &[NatureAddition] {
        &self.additions
    }

    pub fn capture(&self) -> NatureSnapshot {
        NatureSnapshot {
            removed_slots: self.removals.iter().copied().collect(),
            additions: self.additions.clone(),
        }
    }

    pub fn load(&mut self, snapshot: NatureSnapshot) {
        self.removals.clear();
        self.removals.extend(snapshot.removed_slots);
        self.additions = snapshot.additions;
        self.accept_changes();
    }

    pub fn accept_changes(&mut self) {
        self.baseline = self.capture();
        self.clear_history();
    }

    pub fn reset(&mut self) {
        let baseline = self.baseline.clone();
        self.load(baseline);
    }

    pub fn clear(&mut self) {
        self.load(NatureSnapshot::default());
    }

    pub fn plant(&mut self, addition: NatureAddition) {
        self.begin_history_stroke();
        let added = IndexedAddition {
            index: self.additions.len(),
            item: addition.clone(),
        };
        self.additions.push(addition);
        self.record(Operation {
            added: Some(added),
            removed: Vec::new(),
            removed_additions: Vec::new(),
        });
    }

    /// Validate all additions before recording a single layout stroke.
    pub fn plant_many(&mut self, additions: &[NatureAddition]) -> Result<bool, String> {
        for item in additions {
            if item.name.trim().is_empty() {
                return Err("Name must not be empty".to_string());
            }
            if !item.x.is_finite()
                || !item.y.is_finite()
                || !item.z.is_finite()
                || !item.rotation.is_finite()
            {
                return Err("Coordinates and rotation must be finite".to_string());
            }
            if !(0.0..=16383.0).contains(&item.x) || !(0.0..=16383.0).contains(&item.z) {
                return Err(format!(
                    "Invalid coordinates: ({}, {}) must be in [0, 16383]",
                    item.x, item.z
                ));
            }
        }

        if additions.is_empty() {
            return Ok(false);
        }

        self.commit_stroke();
        for item in additions {
            self.plant(item.clone());
        }
        Ok(self.commit_stroke())
    }

    pub fn remove(&mut self, slots: &[i32], additions_to_remove: &[NatureAddition]) -> bool {
        let mut removed = Vec::new();
        for &s in slots {
            if !self.removals.contains(&s) && !removed.contains(&s) {
                removed.push(s);
            }
        }

        let mut remaining: Vec<IndexedAddition> = self
            .additions
            .iter()
            .enumerate()
            .map(|(index, item)| IndexedAddition {
                index,
                item: item.clone(),
            })
            .collect();

        let mut removed_items = Vec::new();
        for target in additions_to_remove {
            if let Some(pos) = remaining.iter().position(|entry| entry.item == *target) {
                removed_items.push(remaining.remove(pos));
            }
        }

        removed_items.sort_by_key(|e| e.index);

        if removed.is_empty() && removed_items.is_empty() {
            return false;
        }

        self.begin_history_stroke();

        for &s in &removed {
            self.removals.insert(s);
        }

        for entry in removed_items.iter().rev() {
            if entry.index < self.additions.len() {
                self.additions.remove(entry.index);
            }
        }

        self.record(Operation {
            added: None,
            removed,
            removed_additions: removed_items,
        });
        true
    }

    pub fn commit_stroke(&mut self) -> bool {
        if self.stroke.is_empty() {
            return false;
        }
        let ops = std::mem::take(&mut self.stroke);
        self.undo_stack.push(ops);
        let changed = self.stroke_before.take().is_some_and(|before| before != self.capture());
        self.history.commit(changed);
        true
    }

    /// 保留既有取消後可重做的行為，但不旁錄取消的筆觸。
    pub fn cancel_stroke(&mut self) -> bool {
        let receipt_count = self.history.receipts.len();
        if !self.commit_stroke() { return false; }
        let changed = self.undo();
        self.history.receipts.truncate(receipt_count);
        changed
    }
    pub fn undo(&mut self) -> bool {
        self.commit_stroke();
        let stroke = match self.undo_stack.pop() {
            Some(s) => s,
            None => return false,
        };

        for op in stroke.iter().rev() {
            if let Some(added) = &op.added {
                if added.index < self.additions.len() {
                    self.additions.remove(added.index);
                }
            }
            for s in &op.removed {
                self.removals.remove(s);
            }
            for entry in &op.removed_additions {
                let idx = entry.index.min(self.additions.len());
                self.additions.insert(idx, entry.item.clone());
            }
        }

        self.redo_stack.push(stroke);
        self.history.undo();
        true
    }

    pub fn redo(&mut self) -> bool {
        self.commit_stroke();
        let stroke = match self.redo_stack.pop() {
            Some(s) => s,
            None => return false,
        };

        for op in &stroke {
            if let Some(added) = &op.added {
                let idx = added.index.min(self.additions.len());
                self.additions.insert(idx, added.item.clone());
            }
            for s in &op.removed {
                self.removals.insert(*s);
            }
            for entry in op.removed_additions.iter().rev() {
                if entry.index < self.additions.len() {
                    self.additions.remove(entry.index);
                }
            }
        }

        self.undo_stack.push(stroke);
        self.history.redo();
        true
    }

    fn record(&mut self, operation: Operation) {
        self.clear_redo();
        self.stroke.push(operation);
    }

    fn begin_history_stroke(&mut self) {
        if self.stroke.is_empty() { self.stroke_before = Some(self.capture()); }
    }

    fn clear_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.stroke.clear();
        self.stroke_before = None;
        self.history = NatureHistory::new(self.history.allocate);
    }
}
