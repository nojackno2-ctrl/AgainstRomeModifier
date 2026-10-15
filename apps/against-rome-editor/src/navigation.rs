//! Read-only diagnostics on the current edited layers; never infer or rewrite
//! the authored collision layer from terrain geometry.
use crate::terrain::TerrainSession;
use against_rome_navigation::{analyze_connectivity, find_path, IsolatedRegion, PassabilityGrid};
use eframe::egui;

#[derive(Default)]
pub struct NavigationUi {
    pub open: bool,
    seed: [i32;2],
    target: [i32;2],
    report: Option<NavigationReport>,
    error: Option<String>,
    snapshot: Option<(Vec<u8>,Vec<u8>,u32)>,
}
pub struct NavigationReport {
    pub passable: usize,
    pub seed_passable: bool,
    pub isolated: Vec<IsolatedRegion>,
    pub path_cells: Option<usize>,
}
pub fn diagnose(session: &TerrainSession, water_surface:f32, seed:[i32;2], target:[i32;2]) -> Result<NavigationReport,String> {
    if !water_surface.is_finite() { return Err("Water surface must be finite".into()); }
    let grid = PassabilityGrid::new(session.collision_size,Some(session.collision()),
        Some((session.vertex_size, session.heights(), 1.0,water_surface)),16384.0)
        .map_err(str::to_owned)?;
    if !grid.in_bounds(seed[0], seed[1]) || !grid.in_bounds(target[0],target[1]) {
        return Err("Seed and destination must lie inside the collision grid".into());
    }
    let passable = (0..grid.size).flat_map(|z|(0..grid.size).map(move|x|(x,z)))
        .filter(|&(x,z)|grid.passable(x as i32,z as i32)).count();
    Ok(NavigationReport {
        passable, seed_passable:grid.passable(seed[0],seed[1]),
        isolated:analyze_connectivity(&grid,&[(seed[0],seed[1])],&[(target[0],target[1])],1),
        path_cells:find_path(&grid,(seed[0],seed[1]),(target[0],target[1])).map(|p|p.len()),
    })
}
impl NavigationUi {
    pub fn invalidate(&mut self) { self.report=None; self.error=None; self.snapshot=None; }
    pub fn show(&mut self,ctx:&egui::Context,session:Option<&TerrainSession>,water_surface:f32) {
        if !self.open {return;}
        if let Some((heights,collision,water))=&self.snapshot {
            if session.is_none_or(|s|s.heights()!=heights || s.collision()!=collision) || *water!=water_surface.to_bits() {
                self.invalidate();
            }
        }
        egui::Window::new("通行診斷 / Navigation").open(&mut self.open).show(ctx,|ui| {
            ui.label("依目前編輯圖層檢查；結果是編輯器診斷，非原遊戲尋路保證。");
            let Some(session)=session else {ui.label("請先載入地圖。");return;};
            ui.label(format!("Collision {} × {} ; water surface {:.2} height samples",session.collision_size,session.collision_size,water_surface));
            let max=(session.collision_size-1) as i32;
            let mut changed=false;
            for (label,point) in [("起點 / Seed",&mut self.seed),("終點 / Destination",&mut self.target)] {
                ui.horizontal(|ui|{ui.label(label);for value in point {changed |= ui.add(egui::DragValue::new(value).clamp_range(0..=max)).changed();}});
            }
            if changed {self.report=None;}
            if ui.button("重新檢查 / Analyze current layers").clicked() {
                match diagnose(session,water_surface,self.seed,self.target) {
                    Ok(report)=>{self.report=Some(report);self.error=None;self.snapshot=Some((session.heights().to_vec(),session.collision().to_vec(),water_surface.to_bits()));},
                    Err(e)=>{self.report=None;self.error=Some(e);},
                }
            }
            if let Some(error)=&self.error {ui.colored_label(egui::Color32::RED,error);}
            if let Some(report)=&self.report {
                ui.label(format!("Passable: {} ; isolated regions from seed: {}",report.passable,report.isolated.len()));
                if !report.seed_passable {ui.label("起點在阻擋格或水下，請選擇其他起點。");}
                ui.label(match report.path_cells {Some(n)=>format!("Path: {n} grid cells"),None=>"No passable path".into()});
                egui::ScrollArea::vertical().max_height(240.0).show(ui,|ui| {
                    for region in &report.isolated {
                        ui.label(format!("{} cells; center ({}, {}); bridge suggestion: {:?}",region.tile_count,region.centroid.x,region.centroid.z,region.bridge.map(|p|(p.x,p.z))));
                    }
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::CollisionBrushOp;
    #[test]
    fn live_diagnostics_follow_edits_and_undo_without_modifying_layers() {
        let mut s=TerrainSession::create_blank(6,32);
        let initial=s.collision().to_vec();
        let r=diagnose(&s,0.0,[0,0],[4,4]).unwrap();
        assert_eq!(r.passable,25); assert!(r.path_cells.is_some());
        for z in 0..5 {s.paint_collision(2.5,z as f32+0.5,0.6,CollisionBrushOp::Block).unwrap();}
        s.commit_stroke();
        let blocked=s.collision().to_vec();
        let r=diagnose(&s,0.0,[0,0],[4,4]).unwrap();
        assert_eq!(r.passable,20);assert!(r.path_cells.is_none());assert_eq!(r.isolated.len(),1);
        assert_eq!(s.collision(),blocked);
        s.undo();assert_eq!(s.collision(),initial);
        assert!(diagnose(&s,0.0,[0,0],[4,4]).unwrap().path_cells.is_some());
        assert!(diagnose(&s,f32::NAN,[0,0],[4,4]).is_err());
        assert!(diagnose(&s,0.0,[-1,0],[4,4]).is_err());
    }
}
