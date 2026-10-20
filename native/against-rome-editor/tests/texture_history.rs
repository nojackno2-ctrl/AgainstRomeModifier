use against_rome_editor::texture_history::*;

fn grid() -> TextureHistory {
    TextureHistory::new(2, vec!["grass".to_string(); 4]).unwrap()
}

#[test]
fn stroke_groups_undo_redo_and_case_insensitive_noop() {
    let mut h = grid();
    assert!(h.paint(0, 0, "GRASS").unwrap().is_none());
    assert!(!h.is_dirty() && !h.can_undo());
    h.paint(1, 0, "sand").unwrap().unwrap();
    h.paint(0, 1, "sand").unwrap();
    assert!(h.can_undo() && h.is_dirty());
    assert!(h.commit_stroke() && !h.commit_stroke());
    let undone = h.undo().unwrap();
    assert_eq!(undone.len(), 2);
    assert!(!h.is_dirty() && h.can_redo());
    h.redo().unwrap();
    assert_eq!(h.current()[1], "sand");
    h.undo().unwrap();
    assert!(h.can_redo());
    h.paint(0, 0, "rock").unwrap();
    h.commit_stroke();
    assert!(!h.can_redo()); // a committed new stroke invalidates redo
}

#[test]
fn reverted_paint_leaves_no_pending_and_order_is_first_seen() {
    let mut h = grid();
    h.paint(0, 0, "a").unwrap();
    h.paint(0, 0, "grass").unwrap(); // back to original
    assert!(!h.can_undo());
    h.paint(1, 1, "b").unwrap();
    h.paint(0, 0, "c").unwrap();
    h.commit_stroke();
    let s = h.undo().unwrap();
    let order: Vec<_> = s.iter().map(|c| (c.x, c.y, c.after.as_str())).collect();
    assert_eq!(order, vec![(0, 0, "c"), (1, 1, "b")]);
    assert_eq!(s[0].before, "grass");
}

#[test]
fn baseline_reset_and_validation() {
    let mut h = grid();
    h.paint(1, 1, "sand").unwrap();
    h.commit_baseline();
    assert!(!h.is_dirty() && h.can_undo());
    h.paint(0, 0, "rock").unwrap();
    let r = h.reset_to_baseline();
    assert_eq!(r.len(), 1);
    assert_eq!((r[0].before.as_str(), r[0].after.as_str()), ("rock", "grass"));
    assert!(!h.can_undo() && !h.can_redo() && !h.is_dirty());
    assert_eq!(h.current()[3], "sand");
    assert!(h.paint(2, 0, "x").is_err() && h.paint(0, 0, "  ").is_err());
    assert!(TextureHistory::new(2, vec![]).is_err() && TextureHistory::new(0, vec![]).is_err());
}

#[test]
fn bresenham_excludes_start_includes_end() {
    assert_eq!(
        stroke_path_between((0, 0), (3, 1)),
        vec![(1, 0), (2, 1), (3, 1)]
    );
    assert!(stroke_path_between((2, 2), (2, 2)).is_empty());
    assert_eq!(stroke_path_between((3, 3), (1, 1)), vec![(2, 2), (1, 1)]);
}
