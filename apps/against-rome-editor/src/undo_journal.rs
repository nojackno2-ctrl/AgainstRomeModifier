//! 編輯器工具列與全域快捷鍵共用的跨子系統歷程。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SubsystemId {
    Terrain,
    Materials,
    Placement,
    Nature,
    Spawns,
    Scenario,
    Scene,
    Other,
    Auxiliary,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubsystemStep {
    pub subsystem: SubsystemId,
    pub before_token: u64,
    pub after_token: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalEntry {
    pub epoch: u64,
    pub children: Vec<SubsystemStep>,
}

/// 回傳 false 必須保持該次呼叫前的狀態；反向操作必須能還原成功的操作。
/// 複合操作失敗會嘗試補償；此 bool 介面無法保證補償失敗時的目標原子性。
pub trait UndoTarget {
    fn token(&self, subsystem: SubsystemId) -> u64;
    fn undo(&mut self, subsystem: SubsystemId) -> bool;
    fn redo(&mut self, subsystem: SubsystemId) -> bool;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UndoJournal {
    epoch: u64,
    undo: Vec<JournalEntry>,
    redo: Vec<JournalEntry>,
}

impl UndoJournal {
    /// 試行測試使用：比較每個子系統目前應在的狀態，不觸發斷言或導航。
    #[cfg(test)]
    pub(crate) fn consistent_tokens(&self, token: impl Fn(SubsystemId) -> u64) -> bool {
        let mut expected = std::collections::HashMap::new();
        for entry in self.redo.iter().rev() {
            for step in &entry.children { expected.entry(step.subsystem).or_insert(step.before_token); }
        }
        for entry in &self.undo {
            for step in &entry.children { expected.insert(step.subsystem, step.after_token); }
        }
        expected.into_iter().all(|(id, value)| token(id) == value)
    }
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// 拒絕過期、空白與重複子系統；每筆子紀錄代表該子系統的一次歷程操作。
    pub fn record(&mut self, entry: JournalEntry) -> bool {
        if entry.epoch != self.epoch
            || entry.children.is_empty()
            || entry.children.iter().enumerate().any(|(i, child)| {
                entry.children[..i]
                    .iter()
                    .any(|prior| prior.subsystem == child.subsystem)
            })
        {
            return false;
        }
        self.redo.clear();
        self.undo.push(entry);
        true
    }

    pub fn record_compound(&mut self, children: Vec<SubsystemStep>) -> bool {
        self.record(JournalEntry {
            epoch: self.epoch,
            children,
        })
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// 導航前驗證最新紀錄，讓呼叫端區分失同步與委派失敗。
    pub fn matches_target(&self, target: &dyn UndoTarget, forward: bool) -> bool {
        let source = if forward { &self.redo } else { &self.undo };
        source.last().is_none_or(|entry| {
            entry.epoch == self.epoch && entry.children.iter().all(|step| {
                target.token(step.subsystem) == if forward { step.before_token } else { step.after_token }
            })
        })
    }
    pub fn undo(&mut self, target: &mut dyn UndoTarget) -> bool {
        self.navigate(target, false)
    }

    pub fn redo(&mut self, target: &mut dyn UndoTarget) -> bool {
        self.navigate(target, true)
    }

    fn navigate(&mut self, target: &mut dyn UndoTarget, forward: bool) -> bool {
        let (source, destination) = if forward {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        let Some(entry) = source.last() else {
            return false;
        };
        if entry.epoch != self.epoch
            || entry.children.iter().any(|child| {
                target.token(child.subsystem)
                    != if forward {
                        child.before_token
                    } else {
                        child.after_token
                    }
            })
        {
            return false;
        }
        let ordered: Vec<_> = if forward {
            entry.children.iter().collect()
        } else {
            entry.children.iter().rev().collect()
        };
        for (index, child) in ordered.iter().enumerate() {
            let success = if forward {
                target.redo(child.subsystem)
            } else {
                target.undo(child.subsystem)
            };
            if !success {
                for completed in ordered[..index].iter().rev() {
                    let restored = if forward {
                        target.undo(completed.subsystem)
                    } else {
                        target.redo(completed.subsystem)
                    };
                    if !restored {
                        break;
                    }
                }
                return false;
            }
        }
        if let Some(entry) = source.pop() {
            destination.push(entry);
        }
        true
    }

    /// 局部導航只移動該子系統最新的單筆紀錄；複合邊界或缺少收據由呼叫端清除紀錄。
    /// 埋藏的獨立步驟移至反向堆疊頂端，保留其餘子系統的時間順序。
    pub fn navigate_subsystem(&mut self, target: &mut dyn UndoTarget, id: SubsystemId, forward: bool) -> Option<bool> {
        let source = if forward { &self.redo } else { &self.undo };
        let index = source.iter().rposition(|entry| entry.children.iter().any(|step| step.subsystem == id))?;
        // Settlement 的 Other 收據包含 DATA／Nature 快照，不能跨越或拆開。
        if matches!(id, SubsystemId::Placement | SubsystemId::Nature)
            && self.undo.iter().chain(&self.redo).any(|entry| entry.children.iter().any(|step| step.subsystem == SubsystemId::Other)) {
            return None;
        }
        let entry = &source[index];
        if entry.children.len() != 1 || entry.epoch != self.epoch { return None; }
        let mut isolated = Self { epoch: self.epoch, ..Default::default() };
        if forward { isolated.redo.push(entry.clone()); } else { isolated.undo.push(entry.clone()); }
        if !isolated.matches_target(target, forward) { return None; }
        if !isolated.navigate(target, forward) { return Some(false); }
        let (source, destination) = if forward { (&mut self.redo, &mut self.undo) } else { (&mut self.undo, &mut self.redo) };
        destination.push(source.remove(index));
        Some(true)
    }
    /// 已由 settlement 原子路由完成操作；核對前後 token 後只移動收據。
    pub fn reconcile_settlement(&mut self, target: &dyn UndoTarget, forward: bool, before: u64) -> bool {
        let (source, destination) = if forward { (&mut self.redo, &mut self.undo) } else { (&mut self.undo, &mut self.redo) };
        let Some(entry) = source.last() else { return false; };
        if entry.epoch != self.epoch || entry.children.len() != 1 { return false; }
        let step = &entry.children[0];
        if step.subsystem != SubsystemId::Other
            || before != if forward { step.before_token } else { step.after_token }
            || target.token(SubsystemId::Other) != if forward { step.after_token } else { step.before_token } {
            return false;
        }
        if let Some(entry) = source.pop() { destination.push(entry); }
        true
    }
    pub(crate) fn checkpoint(&self) -> (u64, usize) {
        (self.epoch, self.undo.len())
    }

    /// 每個子系統必須恰好對應一個本地復原步驟，否則建立障壁。
    pub(crate) fn compound_since(&mut self, checkpoint: (u64, usize)) -> Result<(), String> {
        let (epoch, start) = checkpoint;
        if epoch != self.epoch || start > self.undo.len() {
            self.clear();
            return Err("DATA／外部歷程障壁".into());
        }
        if start == self.undo.len() { return Ok(()); }
        let children: Vec<SubsystemStep> = self.undo.drain(start..).flat_map(|e| e.children).collect();
        for (index, step) in children.iter().enumerate() {
            if children[..index].iter().any(|prior| prior.subsystem == step.subsystem) {
                let reason = format!("{:?} 包含多個本地步驟，無法原子復原", step.subsystem);
                self.clear();
                return Err(reason);
            }
        }
        self.record_compound(children);
        Ok(())
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub fn new_epoch(&mut self) {
        self.clear();
        self.epoch = self.epoch.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use SubsystemId::{Materials, Terrain};

    #[test]
    fn lifecycle_compound_groups_only_new_steps_and_rejects_duplicate_subsystems() {
        let mut journal = UndoJournal::default();
        journal.record_compound(vec![step(Terrain, 0)]);
        let checkpoint = journal.checkpoint();
        journal.record_compound(vec![step(Terrain, 1)]);
        journal.record_compound(vec![step(Materials, 0)]);
        assert!(journal.compound_since(checkpoint).is_ok());
        assert_eq!(journal.undo.len(), 2);
        assert_eq!(journal.undo[1].children, vec![step(Terrain, 1), step(Materials, 0)]);
        let saved = journal.clone();
        assert!(journal.compound_since(journal.checkpoint()).is_ok());
        assert_eq!(journal, saved);
        let checkpoint = journal.checkpoint();
        journal.record_compound(vec![step(Terrain, 2)]);
        journal.record_compound(vec![step(Terrain, 3)]);
        assert!(journal.compound_since(checkpoint).unwrap_err().contains("Terrain"));
        assert!(!journal.can_undo() && !journal.can_redo());
    }

    #[derive(Default)]
    struct Fake {
        tokens: HashMap<SubsystemId, u64>,
        calls: Vec<(bool, SubsystemId)>,
        fail: Option<(bool, SubsystemId)>,
    }

    impl UndoTarget for Fake {
        fn token(&self, id: SubsystemId) -> u64 {
            *self.tokens.get(&id).unwrap_or(&0)
        }
        fn undo(&mut self, id: SubsystemId) -> bool {
            self.calls.push((false, id));
            if self.fail == Some((false, id)) {
                return false;
            }
            self.tokens.insert(id, self.token(id) - 1);
            true
        }
        fn redo(&mut self, id: SubsystemId) -> bool {
            self.calls.push((true, id));
            if self.fail == Some((true, id)) {
                return false;
            }
            self.tokens.insert(id, self.token(id) + 1);
            true
        }
    }

    fn step(subsystem: SubsystemId, before_token: u64) -> SubsystemStep {
        SubsystemStep {
            subsystem,
            before_token,
            after_token: before_token + 1,
        }
    }

    fn compound() -> (UndoJournal, Fake) {
        let mut journal = UndoJournal::default();
        assert!(journal.record_compound(vec![step(Terrain, 0), step(Materials, 0)]));
        let target = Fake {
            tokens: HashMap::from([(Terrain, 1), (Materials, 1)]),
            ..Fake::default()
        };
        (journal, target)
    }

    #[test]
    fn panel_failure_and_compound_leave_journal_unchanged() {
        let (mut journal, mut target) = compound();
        let original = journal.clone();
        assert_eq!(journal.navigate_subsystem(&mut target, Terrain, false), None);
        assert_eq!(journal, original);
        assert!(target.calls.is_empty());

        journal.clear();
        assert!(journal.record_compound(vec![step(Terrain, 0)]));
        assert!(journal.record_compound(vec![step(Materials, 0)]));
        let original = journal.clone();
        target.fail = Some((false, Terrain));
        assert_eq!(journal.navigate_subsystem(&mut target, Terrain, false), Some(false));
        assert_eq!(journal, original);
        assert_eq!(target.calls, vec![(false, Terrain)]);
        target.fail = None;
        assert_eq!(journal.navigate_subsystem(&mut target, Terrain, false), Some(true));
        let original = journal.clone();
        target.fail = Some((true, Terrain));
        assert_eq!(journal.navigate_subsystem(&mut target, Terrain, true), Some(false));
        assert_eq!(journal, original);
    }
    #[test]
    fn chronological_order_across_subsystems() {
        let mut j = UndoJournal::default();
        let mut t = Fake::default();
        for id in [Terrain, Materials, Terrain] {
            let before = t.token(id);
            t.tokens.insert(id, before + 1);
            assert!(j.record_compound(vec![step(id, before)]));
        }
        for _ in 0..3 {
            assert!(j.undo(&mut t));
        }
        assert!(!j.undo(&mut t));
        for _ in 0..3 {
            assert!(j.redo(&mut t));
        }
        assert_eq!(
            t.calls,
            vec![
                (false, Terrain),
                (false, Materials),
                (false, Terrain),
                (true, Terrain),
                (true, Materials),
                (true, Terrain)
            ]
        );
    }

    #[test]
    fn mismatch_preflights_every_child_in_both_directions() {
        let (mut j, mut t) = compound();
        t.tokens.insert(Terrain, 99);
        let saved = j.clone();
        assert!(!j.undo(&mut t));
        assert_eq!(j, saved);
        assert!(t.calls.is_empty());
        t.tokens.insert(Terrain, 1);
        assert!(j.undo(&mut t));
        t.calls.clear();
        t.tokens.insert(Materials, 99);
        let saved = j.clone();
        assert!(!j.redo(&mut t));
        assert_eq!(j, saved);
        assert!(t.calls.is_empty());
    }

    #[test]
    fn failed_delegation_never_falls_through() {
        let (mut j, mut t) = compound();
        t.fail = Some((false, Materials));
        let saved = j.clone();
        assert!(!j.undo(&mut t));
        assert_eq!(j, saved);
        assert_eq!(t.calls, vec![(false, Materials)]);
    }

    #[test]
    fn compounds_undo_reverse_and_redo_forward() {
        let (mut j, mut t) = compound();
        assert!(j.undo(&mut t));
        assert!(!j.can_undo());
        assert!(j.can_redo());
        assert!(j.redo(&mut t));
        assert_eq!(
            t.calls,
            vec![
                (false, Materials),
                (false, Terrain),
                (true, Terrain),
                (true, Materials)
            ]
        );
    }

    #[test]
    fn compound_failure_compensates_and_preserves_journal() {
        for forward in [false, true] {
            let (mut j, mut t) = compound();
            if forward {
                assert!(j.undo(&mut t));
            }
            t.calls.clear();
            t.fail = Some((forward, if forward { Materials } else { Terrain }));
            let saved = j.clone();
            let tokens = t.tokens.clone();
            assert!(!if forward {
                j.redo(&mut t)
            } else {
                j.undo(&mut t)
            });
            assert_eq!(j, saved);
            assert_eq!(t.tokens, tokens);
            let first = if forward { Terrain } else { Materials };
            let second = if forward { Materials } else { Terrain };
            assert_eq!(
                t.calls,
                vec![(forward, first), (forward, second), (!forward, first)]
            );
        }
    }

    #[test]
    fn new_record_clears_redo() {
        let (mut j, mut t) = compound();
        assert!(j.undo(&mut t));
        assert!(j.record_compound(vec![step(Terrain, 0)]));
        assert!(!j.can_redo());
    }

    #[test]
    fn clear_and_epoch_discard_both_stacks_and_reject_stale_entries() {
        let (mut j, mut t) = compound();
        assert!(j.undo(&mut t));
        j.clear();
        assert_eq!(j.epoch(), 0);
        assert!(!j.can_undo() && !j.can_redo());
        assert!(j.record_compound(vec![step(Terrain, 0)]));
        j.new_epoch();
        assert_eq!(j.epoch(), 1);
        assert!(!j.can_undo() && !j.can_redo());
        assert!(!j.record(JournalEntry {
            epoch: 0,
            children: vec![step(Terrain, 0)]
        }));
        assert!(j.record_compound(vec![step(Terrain, 0)]));
    }

    #[test]
    fn invalid_compounds_preserve_redo() {
        let (mut j, mut t) = compound();
        assert!(j.undo(&mut t));
        let saved = j.clone();
        assert!(!j.record_compound(vec![]));
        assert!(!j.record_compound(vec![step(Terrain, 0), step(Terrain, 1)]));
        assert_eq!(j, saved);
    }
}
