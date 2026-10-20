use against_rome_navigation::*;

/// Snapshot-based in-memory session: each commit pushes the pre-stroke state, undo restores it.
struct Mem {
    dim: usize,
    tex: Vec<String>,
    col_size: usize,
    col: Vec<u8>,
    tex_base: Vec<String>,
    col_base: Vec<u8>,
    tex_undo: Vec<Vec<String>>,
    col_undo: Vec<Vec<u8>>,
}
impl Mem {
    fn new(dim: usize, col_size: usize, fill: u8) -> Self {
        let tex = vec!["GRAS".to_string(); dim * dim];
        let col = vec![fill; col_size * col_size];
        Mem {
            dim,
            tex_base: tex.clone(),
            col_base: col.clone(),
            tex,
            col_size,
            col,
            tex_undo: vec![],
            col_undo: vec![],
        }
    }
}
impl RepairSession for Mem {
    fn tile_dimension(&self) -> usize {
        self.dim
    }
    fn stamp_texture(&mut self, x: i32, z: i32, t: &str) -> bool {
        let i = z as usize * self.dim + x as usize;
        if self.tex[i] == t {
            return false;
        }
        self.tex[i] = t.into();
        true
    }
    fn commit_texture_stroke(&mut self) {
        self.tex_undo.push(std::mem::replace(&mut self.tex_base, self.tex.clone()));
    }
    fn undo_texture_stroke(&mut self) {
        self.tex = self.tex_undo.pop().unwrap();
        self.tex_base = self.tex.clone();
    }
    fn collision_size(&self) -> usize {
        self.col_size
    }
    fn paint_collision(&mut self, px: usize, pz: usize, clear: bool) -> bool {
        let i = pz * self.col_size + px;
        let new = if clear { 0 } else { 1 };
        if self.col[i] == new {
            return false;
        }
        self.col[i] = new;
        true
    }
    fn commit_collision_stroke(&mut self) {
        self.col_undo.push(std::mem::replace(&mut self.col_base, self.col.clone()));
    }
    fn undo_collision_stroke(&mut self) {
        self.col = self.col_undo.pop().unwrap();
        self.col_base = self.col.clone();
    }
}

fn action(changes: Vec<TileChange>) -> RepairAction {
    RepairAction {
        id: "t".into(),
        kind: IssueKind::RoadGap,
        title_zh: String::new(),
        title_en: String::new(),
        description_zh: String::new(),
        description_en: String::new(),
        changes,
        focus: Coordinate::from_tile(0, 0, 4, 16384.0),
    }
}
fn tc(x: i32, z: i32, t: &str, c: Option<u8>) -> TileChange {
    TileChange { x, z, texture: t.into(), collision: c }
}

#[test]
fn clears_every_collision_pixel_in_tile_only() {
    let mut m = Mem::new(4, 16, 1); // step 4
    let out = apply_repair(&action(vec![tc(1, 2, "H_WEG1", Some(0))]), &mut m).unwrap();
    assert_eq!(out, RepairOutcome { texture_committed: true, collision_committed: true });
    assert_eq!(m.tex[2 * 4 + 1], "H_WEG1");
    assert_eq!(m.col.iter().filter(|&&v| v == 0).count(), 16);
    for pz in 8..12 {
        for px in 4..8 {
            assert_eq!(m.col[pz * 16 + px], 0);
        }
    }
}

#[test]
fn out_of_range_tile_rejected_without_mutation() {
    let mut m = Mem::new(4, 8, 1);
    let a = action(vec![tc(0, 0, "H_WEG1", Some(0)), tc(4, 0, "H_WEG1", None)]);
    assert_eq!(apply_repair(&a, &mut m), Err(RepairError::TileOutOfRange { x: 4, z: 0 }));
    assert_eq!(m.tex[0], "GRAS");
    assert!(m.col.iter().all(|&v| v == 1));
    assert!(m.tex_undo.is_empty() && m.col_undo.is_empty());
    assert!(apply_repair(&action(vec![tc(-1, 0, "X", None)]), &mut m).is_err());
}

#[test]
fn no_collision_layer_and_blank_texture_are_skipped() {
    let mut m = Mem::new(4, 0, 1);
    let out = apply_repair(&action(vec![tc(0, 0, "  ", Some(0))]), &mut m).unwrap();
    assert!(!out.changed());
    assert!(m.tex_undo.is_empty());
    let out = apply_repair(&action(vec![tc(0, 0, "H_WEG1", Some(0))]), &mut m).unwrap();
    assert_eq!(out, RepairOutcome { texture_committed: true, collision_committed: false });
}

#[test]
fn noop_repair_commits_nothing_and_zero_dim_defaults_to_64() {
    let mut m = Mem::new(64, 0, 0);
    m.dim = 0;
    let out = apply_repair(&action(vec![tc(63, 63, "GRAS", None)]), &mut m).unwrap();
    assert!(!out.changed());
    assert!(m.tex_undo.is_empty());
    assert!(apply_repair(&action(vec![tc(64, 0, "GRAS", None)]), &mut m).is_err());
}

#[test]
fn undo_reverts_committed_strokes_and_empty_outcome_is_noop() {
    let mut m = Mem::new(4, 8, 1);
    let (tex0, col0) = (m.tex.clone(), m.col.clone());
    let out = apply_repair(&action(vec![tc(0, 0, "H_WEG1", Some(0))]), &mut m).unwrap();
    assert_ne!(m.tex, tex0);
    assert_ne!(m.col, col0);
    undo_repair(out, &mut m);
    assert_eq!(m.tex, tex0);
    assert_eq!(m.col, col0);
    undo_repair(RepairOutcome::default(), &mut m);
    assert!(m.tex_undo.is_empty() && m.col_undo.is_empty());
}

#[test]
fn road_heal_action_round_trips_through_session() {
    let gap = RoadGap {
        start: Coordinate::from_tile(0, 0, 4, 16384.0),
        end: Coordinate::from_tile(3, 0, 4, 16384.0),
        tiles: vec![
            Coordinate::from_tile(1, 0, 4, 16384.0),
            Coordinate::from_tile(2, 0, 4, 16384.0),
        ],
        texture: "H_WEG1",
        style: "road",
        clear_collision: true,
    };
    let actions = road_heal_actions(&[gap]);
    let mut m = Mem::new(4, 4, 1);
    let tex0 = m.tex.clone();
    let out = apply_repair(&actions[0], &mut m).unwrap();
    assert_eq!(&m.tex[1..3], ["H_WEG1", "H_WEG1"]);
    assert_eq!(&m.col[1..3], [0, 0]);
    undo_repair(out, &mut m);
    assert_eq!(m.tex, tex0);
}
