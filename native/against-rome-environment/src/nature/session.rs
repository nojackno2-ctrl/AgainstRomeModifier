//! NatureEditSession: Pending nature object modifications, stroke history, undo and redo.

use super::engine::NatureAddition;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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
}

impl Default for NatureEditSession {
    fn default() -> Self {
        Self::new()
    }
}

impl NatureEditSession {
    pub fn new() -> Self {
        Self {
            removals: BTreeSet::new(),
            additions: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            stroke: Vec::new(),
            baseline: NatureSnapshot::default(),
        }
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
        true
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
        true
    }

    fn record(&mut self, operation: Operation) {
        self.redo_stack.clear();
        self.stroke.push(operation);
    }

    fn clear_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.stroke.clear();
    }
}
