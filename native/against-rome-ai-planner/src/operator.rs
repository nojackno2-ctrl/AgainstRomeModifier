//! AiEditorOperator: translates AiMapFeatures into editor brush operations.

use against_rome_editor::terrain::TerrainSession;
use against_rome_textures::region::TerrainRegionPlanner;
use against_rome_textures::session::{TerrainBlendEditSession, TerrainTextureChange};
use crate::brush::{
    EditorBrushExecutor, EditorBrushResult, EditorBrushStroke, EditorBrushTool, EditorHeightBrush,
};
use crate::plan::{AiMapApplyResult, AiMapPlan};

pub struct AiEditorOperator;

struct OperatorCounters {
    total_height_samples: usize,
    total_collision_pixels: usize,
    total_material_strokes: usize,
    total_rejected_material_strokes: usize,
    operation_logs: Vec<String>,
}

fn run_stroke(
    stroke: &EditorBrushStroke,
    heights: &mut Option<&mut TerrainSession>,
    blend: &mut Option<&mut TerrainBlendEditSession>,
    dimension: usize,
    water_level_sample: f32,
    commit_per_stroke: bool,
    allows_tile: Option<&dyn Fn(i32, i32) -> bool>,
    paint_material_fallback: Option<&dyn Fn(&str, f32, f32, f32) -> bool>,
    on_texture: &mut Option<&mut dyn FnMut(TerrainTextureChange)>,
    on_rejected_material: &mut Option<&mut dyn FnMut(f32, f32, f32)>,
    counters: &mut OperatorCounters,
) -> EditorBrushResult {
    let tex_cb = on_texture.as_mut().map(|cb| &mut **cb as &mut dyn FnMut(TerrainTextureChange));
    let radius = stroke.radius_tiles();
    let mut rejected_cb = |x: i32, y: i32| {
        if let Some(cb) = on_rejected_material.as_mut() {
            cb(x as f32 + 0.5, y as f32 + 0.5, radius);
        }
    };
    let result = EditorBrushExecutor::execute(
        stroke,
        heights.as_deref_mut(),
        blend.as_deref_mut(),
        dimension,
        water_level_sample,
        0,
        allows_tile,
        paint_material_fallback,
        tex_cb,
        Some(&mut rejected_cb),
    );

    if stroke.tool == EditorBrushTool::Material {
        counters.total_material_strokes += 1;
        if result.rejected_material_dabs > 0 {
            counters.total_rejected_material_strokes += 1;
        }
    }

    if commit_per_stroke {
        if let Some(h) = heights.as_deref_mut() {
            h.commit_stroke();
        }
        if let Some(b) = blend.as_deref_mut() {
            b.commit_stroke();
        }
    }

    counters.total_height_samples += result.height_samples_changed;
    counters.total_collision_pixels += result.collision_pixels_changed;

    let command = AiEditorOperator::format_brush_command(stroke);
    let human = AiEditorOperator::format_human_description(stroke, &result);
    counters
        .operation_logs
        .push(format!("{}. {}\r\n   {}", counters.operation_logs.len() + 1, human, command));

    result
}

impl AiEditorOperator {
    pub fn apply(
        plan: &AiMapPlan,
        mut heights: Option<&mut TerrainSession>,
        mut blend: Option<&mut TerrainBlendEditSession>,
        dimension: usize,
        water_level_sample: f32,
        commit_per_stroke: bool,
        paint_material_fallback: Option<&dyn Fn(&str, f32, f32, f32) -> bool>,
        mut on_texture: Option<&mut dyn FnMut(TerrainTextureChange)>,
        mut on_rejected_material: Option<&mut dyn FnMut(f32, f32, f32)>,
    ) -> AiMapApplyResult {
        let allows_tile_fn = |x: i32, y: i32| -> bool {
            match &plan.edit_scope {
                Some(s) => s.allows_tile(x, y),
                None => true,
            }
        };
        let allows_tile_ref: Option<&dyn Fn(i32, i32) -> bool> = if plan.edit_scope.is_some() {
            Some(&allows_tile_fn)
        } else {
            None
        };

        let mut counters = OperatorCounters {
            total_height_samples: 0,
            total_collision_pixels: 0,
            total_material_strokes: 0,
            total_rejected_material_strokes: 0,
            operation_logs: Vec::new(),
        };

        // 1. baseHeight
        if let Some(base_height) = plan.base_height {
            let target = base_height.clamp(0, 255);
            let mut min_x = 0i32;
            let mut min_y = 0i32;
            let mut max_x = dimension as i32 - 1;
            let mut max_y = dimension as i32 - 1;

            if let Some(scope) = &plan.edit_scope {
                min_x = scope.bounds().x.clamp(0, dimension as i32 - 1);
                min_y = scope.bounds().y.clamp(0, dimension as i32 - 1);
                max_x = (scope.bounds().right() - 1).clamp(0, dimension as i32 - 1);
                max_y = (scope.bounds().bottom() - 1).clamp(0, dimension as i32 - 1);
            }

            let mid_x = (min_x + max_x) / 2;
            let mid_y = (min_y + max_y) / 2;
            let mut ref_point = (mid_x, mid_y);

            if let Some(allows) = allows_tile_ref {
                if !allows(mid_x, mid_y) {
                    'found_ref: for y in min_y..=max_y {
                        for x in min_x..=max_x {
                            if allows(x, y) {
                                ref_point = (x, y);
                                break 'found_ref;
                            }
                        }
                    }
                }
            }

            for _ in 0..40 {
                let curr = Self::get_center_height(heights.as_deref(), ref_point.0, ref_point.1, dimension);
                let diff = target - curr;
                if diff.abs() <= 1 {
                    break;
                }
                let strength = if diff.abs() >= 10 {
                    2
                } else if diff.abs() >= 4 {
                    1
                } else {
                    0
                };
                let op = if diff > 0 {
                    EditorHeightBrush::Raise
                } else {
                    EditorHeightBrush::Lower
                };

                let mut stroke = EditorBrushStroke::new(EditorBrushTool::Height, vec![ref_point]);
                stroke.size = 15;
                stroke.strength_level = strength;
                stroke.height_operation = op;

                let res = run_stroke(
                    &stroke,
                    &mut heights,
                    &mut blend,
                    dimension,
                    water_level_sample,
                    commit_per_stroke,
                    allows_tile_ref,
                    paint_material_fallback,
                    &mut on_texture,
                    &mut on_rejected_material,
                    &mut counters,
                );
                if res.height_samples_changed == 0 {
                    break;
                }
            }

            let mut flatten_path = vec![ref_point];
            flatten_path.extend(Self::generate_serpentine_path(min_x, min_y, max_x, max_y, 6));

            let mut flatten_stroke = EditorBrushStroke::new(EditorBrushTool::Height, flatten_path);
            flatten_stroke.size = 15;
            flatten_stroke.strength_level = 2;
            flatten_stroke.height_operation = EditorHeightBrush::Flatten;

            run_stroke(
                &flatten_stroke,
                &mut heights,
                &mut blend,
                dimension,
                water_level_sample,
                commit_per_stroke,
                allows_tile_ref,
                paint_material_fallback,
                &mut on_texture,
                &mut on_rejected_material,
                &mut counters,
            );
            run_stroke(
                &flatten_stroke,
                &mut heights,
                &mut blend,
                dimension,
                water_level_sample,
                commit_per_stroke,
                allows_tile_ref,
                paint_material_fallback,
                &mut on_texture,
                &mut on_rejected_material,
                &mut counters,
            );
        }

        // 2. baseMaterial
        if let Some(base_mat) = plan.base_material.as_ref() {
            let mut min_x = 0i32;
            let mut min_y = 0i32;
            let mut max_x = dimension as i32 - 1;
            let mut max_y = dimension as i32 - 1;

            if let Some(scope) = &plan.edit_scope {
                min_x = scope.bounds().x.clamp(0, dimension as i32 - 1);
                min_y = scope.bounds().y.clamp(0, dimension as i32 - 1);
                max_x = (scope.bounds().right() - 1).clamp(0, dimension as i32 - 1);
                max_y = (scope.bounds().bottom() - 1).clamp(0, dimension as i32 - 1);
            }

            if blend.is_some() {
                let rect_cells = TerrainRegionPlanner::rectangle(
                    dimension,
                    (min_x as usize, min_y as usize),
                    (max_x as usize, max_y as usize),
                )
                .unwrap_or_default();

                let cells: Vec<(usize, usize)> = rect_cells
                    .into_iter()
                    .filter(|&(x, y)| match allows_tile_ref {
                        Some(allows) => allows(x as i32, y as i32),
                        None => true,
                    })
                    .collect();

                counters.total_material_strokes += 1;
                let fill_res = blend
                    .as_deref_mut()
                    .unwrap()
                    .paint_tiles(&cells, base_mat, false);

                match fill_res {
                    Ok(fill) => {
                        if !fill.succeeded {
                            counters.total_rejected_material_strokes += 1;
                            if let Some(cb) = on_rejected_material.as_deref_mut() {
                                for issue in &fill.issues {
                                    cb(issue.tile_x as f32 + 0.5, issue.tile_y as f32 + 0.5, 0.5);
                                }
                            }
                        }
                        for change in &fill.texture_changes {
                            if let Some(cb) = on_texture.as_deref_mut() {
                                cb(change.clone());
                            }
                        }
                        if commit_per_stroke {
                            if let Some(b) = blend.as_deref_mut() {
                                b.commit_stroke();
                            }
                        }
                        let log_entry = format!(
                            "{}. 區域工具：矩形填色 ({},{})–({},{})，材質 {}（{}）",
                            counters.operation_logs.len() + 1,
                            min_x,
                            min_y,
                            max_x,
                            max_y,
                            base_mat,
                            if fill.succeeded {
                                format!("{} 格", fill.texture_changes.len())
                            } else {
                                "缺少原版接縫，未修改".to_string()
                            }
                        );
                        counters.operation_logs.push(log_entry);
                    }
                    Err(_) => {
                        counters.total_rejected_material_strokes += 1;
                        let log_entry = format!(
                            "{}. 區域工具：矩形填色 ({},{})–({},{})，材質 {}（失敗）",
                            counters.operation_logs.len() + 1,
                            min_x,
                            min_y,
                            max_x,
                            max_y,
                            base_mat
                        );
                        counters.operation_logs.push(log_entry);
                    }
                }
            } else if paint_material_fallback.is_some() {
                let mut stroke = EditorBrushStroke::new(
                    EditorBrushTool::Material,
                    Self::generate_serpentine_path(min_x, min_y, max_x, max_y, 6),
                );
                stroke.size = 15;
                stroke.material = Some(base_mat.clone());

                run_stroke(
                    &stroke,
                    &mut heights,
                    &mut blend,
                    dimension,
                    water_level_sample,
                    commit_per_stroke,
                    allows_tile_ref,
                    paint_material_fallback,
                    &mut on_texture,
                    &mut on_rejected_material,
                    &mut counters,
                );
            }
        }

        // 3. Each feature
        for feature in &plan.features {
            let ftype = match feature.feature_type.as_deref() {
                Some(t) => t,
                None => continue,
            };

            match ftype {
                "hill" | "mountain" | "valley" => {
                    let is_valley = ftype == "valley";
                    let default_amount = if ftype == "mountain" {
                        110
                    } else if ftype == "hill" {
                        45
                    } else {
                        40
                    };
                    let amount = feature.amount.unwrap_or(default_amount);
                    let cx = feature.x;
                    let cy = feature.y;
                    let h0 = Self::get_center_height(heights.as_deref(), cx, cy, dimension);
                    let target_gain = amount;

                    for _ in 0..40 {
                        let curr = Self::get_center_height(heights.as_deref(), cx, cy, dimension);
                        let current_gain = if is_valley { h0 - curr } else { curr - h0 };
                        if current_gain >= target_gain {
                            break;
                        }
                        let remaining = target_gain - current_gain;
                        let progress = (current_gain as f32 / 1.0f32.max(target_gain as f32)).clamp(0.0, 1.0);
                        let current_radius = (feature.radius as f32 * (1.0 - progress * 0.75)).round() as i32;
                        let current_radius = current_radius.max(1);
                        let brush_size = Self::choose_brush_size(current_radius);
                        let strength = if remaining >= 10 {
                            2
                        } else if remaining >= 4 {
                            1
                        } else {
                            0
                        };

                        let op = if is_valley {
                            EditorHeightBrush::Lower
                        } else {
                            EditorHeightBrush::Raise
                        };

                        let mut stroke = EditorBrushStroke::new(EditorBrushTool::Height, vec![(cx, cy)]);
                        stroke.size = brush_size;
                        stroke.strength_level = strength;
                        stroke.height_operation = op;

                        let res = run_stroke(
                            &stroke,
                            &mut heights,
                            &mut blend,
                            dimension,
                            water_level_sample,
                            commit_per_stroke,
                            allows_tile_ref,
                            paint_material_fallback,
                            &mut on_texture,
                            &mut on_rejected_material,
                            &mut counters,
                        );
                        if res.height_samples_changed == 0 {
                            break;
                        }
                    }

                    // Smooth pass
                    let smooth_size = Self::choose_brush_size(feature.radius);
                    let mut smooth_stroke = EditorBrushStroke::new(EditorBrushTool::Height, vec![(cx, cy)]);
                    smooth_stroke.size = smooth_size;
                    smooth_stroke.strength_level = 0;
                    smooth_stroke.height_operation = EditorHeightBrush::Smooth;

                    run_stroke(
                        &smooth_stroke,
                        &mut heights,
                        &mut blend,
                        dimension,
                        water_level_sample,
                        commit_per_stroke,
                        allows_tile_ref,
                        paint_material_fallback,
                        &mut on_texture,
                        &mut on_rejected_material,
                        &mut counters,
                    );
                }

                "ridge" => {
                    let amount = feature.amount.unwrap_or(70);
                    let x1 = feature.x;
                    let y1 = feature.y;
                    let x2 = feature.x2.unwrap_or(x1);
                    let y2 = feature.y2.unwrap_or(y1);
                    let mx = (x1 + x2) / 2;
                    let my = (y1 + y2) / 2;
                    let h0 = Self::get_center_height(heights.as_deref(), mx, my, dimension);
                    let brush_size = Self::choose_brush_size(feature.radius);
                    let path = vec![(x1, y1), (x2, y2)];

                    for _ in 0..40 {
                        let curr = Self::get_center_height(heights.as_deref(), mx, my, dimension);
                        let gained = curr - h0;
                        if gained >= amount {
                            break;
                        }
                        let remaining = amount - gained;
                        let strength = if remaining >= 10 {
                            2
                        } else if remaining >= 4 {
                            1
                        } else {
                            0
                        };

                        let mut stroke = EditorBrushStroke::new(EditorBrushTool::Height, path.clone());
                        stroke.size = brush_size;
                        stroke.strength_level = strength;
                        stroke.height_operation = EditorHeightBrush::Raise;

                        let res = run_stroke(
                            &stroke,
                            &mut heights,
                            &mut blend,
                            dimension,
                            water_level_sample,
                            commit_per_stroke,
                            allows_tile_ref,
                            paint_material_fallback,
                            &mut on_texture,
                            &mut on_rejected_material,
                            &mut counters,
                        );
                        if res.height_samples_changed == 0 {
                            break;
                        }
                    }

                    let mut smooth_stroke = EditorBrushStroke::new(EditorBrushTool::Height, path);
                    smooth_stroke.size = brush_size;
                    smooth_stroke.strength_level = 0;
                    smooth_stroke.height_operation = EditorHeightBrush::Smooth;

                    run_stroke(
                        &smooth_stroke,
                        &mut heights,
                        &mut blend,
                        dimension,
                        water_level_sample,
                        commit_per_stroke,
                        allows_tile_ref,
                        paint_material_fallback,
                        &mut on_texture,
                        &mut on_rejected_material,
                        &mut counters,
                    );
                }

                "plateau" => {
                    let target = feature.amount.unwrap_or(150);
                    let cx = feature.x;
                    let cy = feature.y;
                    let brush_size = Self::choose_brush_size(feature.radius);

                    for _ in 0..40 {
                        let curr = Self::get_center_height(heights.as_deref(), cx, cy, dimension);
                        let diff = target - curr;
                        if diff.abs() <= 1 {
                            break;
                        }
                        let strength = if diff.abs() >= 10 {
                            2
                        } else if diff.abs() >= 4 {
                            1
                        } else {
                            0
                        };
                        let op = if diff > 0 {
                            EditorHeightBrush::Raise
                        } else {
                            EditorHeightBrush::Lower
                        };

                        let mut stroke = EditorBrushStroke::new(EditorBrushTool::Height, vec![(cx, cy)]);
                        stroke.size = brush_size;
                        stroke.strength_level = strength;
                        stroke.height_operation = op;

                        let res = run_stroke(
                            &stroke,
                            &mut heights,
                            &mut blend,
                            dimension,
                            water_level_sample,
                            commit_per_stroke,
                            allows_tile_ref,
                            paint_material_fallback,
                            &mut on_texture,
                            &mut on_rejected_material,
                            &mut counters,
                        );
                        if res.height_samples_changed == 0 {
                            break;
                        }
                    }

                    let mut flatten_list = vec![(cx, cy)];
                    if feature.radius > brush_size / 2 + 1 {
                        flatten_list.extend(Self::generate_circle_serpentine_path(
                            cx,
                            cy,
                            feature.radius,
                            (brush_size as f32 * 0.75).round() as i32,
                            dimension,
                        ));
                    }

                    let mut flatten_stroke = EditorBrushStroke::new(EditorBrushTool::Height, flatten_list);
                    flatten_stroke.size = brush_size;
                    flatten_stroke.strength_level = 2;
                    flatten_stroke.height_operation = EditorHeightBrush::Flatten;

                    run_stroke(
                        &flatten_stroke,
                        &mut heights,
                        &mut blend,
                        dimension,
                        water_level_sample,
                        commit_per_stroke,
                        allows_tile_ref,
                        paint_material_fallback,
                        &mut on_texture,
                        &mut on_rejected_material,
                        &mut counters,
                    );
                    run_stroke(
                        &flatten_stroke,
                        &mut heights,
                        &mut blend,
                        dimension,
                        water_level_sample,
                        commit_per_stroke,
                        allows_tile_ref,
                        paint_material_fallback,
                        &mut on_texture,
                        &mut on_rejected_material,
                        &mut counters,
                    );
                }

                "lake" => {
                    let cx = feature.x;
                    let cy = feature.y;
                    let brush_size = Self::choose_brush_size(feature.radius);
                    let mut path = vec![(cx, cy)];
                    if feature.radius > brush_size / 2 + 1 {
                        path.extend(Self::generate_circle_serpentine_path(
                            cx,
                            cy,
                            feature.radius,
                            (brush_size as f32 * 0.75).round() as i32,
                            dimension,
                        ));
                    }

                    let mut stroke = EditorBrushStroke::new(EditorBrushTool::Height, path);
                    stroke.size = brush_size;
                    stroke.strength_level = 2;
                    stroke.height_operation = EditorHeightBrush::Water;
                    stroke.water_depth = Some(feature.amount.unwrap_or(16).clamp(5, 30));

                    run_stroke(
                        &stroke,
                        &mut heights,
                        &mut blend,
                        dimension,
                        water_level_sample,
                        commit_per_stroke,
                        allows_tile_ref,
                        paint_material_fallback,
                        &mut on_texture,
                        &mut on_rejected_material,
                        &mut counters,
                    );
                }

                "river" => {
                    let brush_size = Self::choose_brush_size(feature.radius);
                    let x1 = feature.x;
                    let y1 = feature.y;
                    let x2 = feature.x2.unwrap_or(x1);
                    let y2 = feature.y2.unwrap_or(y1);
                    let path = vec![(x1, y1), (x2, y2)];

                    let mut stroke = EditorBrushStroke::new(EditorBrushTool::Height, path);
                    stroke.size = brush_size;
                    stroke.strength_level = 2;
                    stroke.height_operation = EditorHeightBrush::Water;
                    stroke.water_depth = Some(feature.amount.unwrap_or(16).clamp(5, 30));

                    run_stroke(
                        &stroke,
                        &mut heights,
                        &mut blend,
                        dimension,
                        water_level_sample,
                        commit_per_stroke,
                        allows_tile_ref,
                        paint_material_fallback,
                        &mut on_texture,
                        &mut on_rejected_material,
                        &mut counters,
                    );
                }

                "material" => {
                    if let Some(mat) = feature.material.as_ref() {
                        let brush_size = Self::choose_brush_size(feature.radius);
                        let mut path = vec![(feature.x, feature.y)];
                        if feature.radius > brush_size / 2 + 1 {
                            path.extend(Self::generate_circle_serpentine_path(
                                feature.x,
                                feature.y,
                                feature.radius,
                                (brush_size as f32 * 0.75).round() as i32,
                                dimension,
                            ));
                        }

                        let mut stroke = EditorBrushStroke::new(EditorBrushTool::Material, path);
                        stroke.size = brush_size;
                        stroke.material = Some(mat.clone());

                        run_stroke(
                            &stroke,
                            &mut heights,
                            &mut blend,
                            dimension,
                            water_level_sample,
                            commit_per_stroke,
                            allows_tile_ref,
                            paint_material_fallback,
                            &mut on_texture,
                            &mut on_rejected_material,
                            &mut counters,
                        );
                    }
                }

                "blocked" | "passable" => {
                    let should_edit = plan
                        .edit_scope
                        .as_ref()
                        .map_or(true, |s| s.edit_passability());
                    if should_edit {
                        let block = ftype == "blocked";
                        let brush_size = Self::choose_brush_size(feature.radius);
                        let path = if let (Some(x2), Some(y2)) = (feature.x2, feature.y2) {
                            vec![(feature.x, feature.y), (x2, y2)]
                        } else if feature.radius <= brush_size / 2 + 1 {
                            vec![(feature.x, feature.y)]
                        } else {
                            Self::generate_circle_serpentine_path(
                                feature.x,
                                feature.y,
                                feature.radius,
                                (brush_size as f32 * 0.75).round() as i32,
                                dimension,
                            )
                        };

                        let mut stroke = EditorBrushStroke::new(EditorBrushTool::Collision, path);
                        stroke.size = brush_size;
                        stroke.block = block;

                        run_stroke(
                            &stroke,
                            &mut heights,
                            &mut blend,
                            dimension,
                            water_level_sample,
                            commit_per_stroke,
                            allows_tile_ref,
                            paint_material_fallback,
                            &mut on_texture,
                            &mut on_rejected_material,
                            &mut counters,
                        );
                    }
                }

                _ => {}
            }

            // Attached material for non-material features
            if let Some(mat) = feature.material.as_ref() {
                if ftype != "material" {
                    let linear = ftype == "river"
                        || ftype == "ridge"
                        || ftype == "blocked"
                        || ftype == "passable";
                    let stroke = if linear && feature.x2.is_some() && feature.y2.is_some() {
                        let brush_size = Self::choose_brush_size(feature.radius + 1);
                        let path = vec![
                            (feature.x, feature.y),
                            (feature.x2.unwrap(), feature.y2.unwrap()),
                        ];
                        let mut s = EditorBrushStroke::new(EditorBrushTool::Material, path);
                        s.size = brush_size;
                        s.material = Some(mat.clone());
                        s
                    } else {
                        let brush_size = Self::choose_brush_size(feature.radius);
                        let mut path = vec![(feature.x, feature.y)];
                        if feature.radius > brush_size / 2 + 1 {
                            path.extend(Self::generate_circle_serpentine_path(
                                feature.x,
                                feature.y,
                                feature.radius,
                                (brush_size as f32 * 0.75).round() as i32,
                                dimension,
                            ));
                        }
                        let mut s = EditorBrushStroke::new(EditorBrushTool::Material, path);
                        s.size = brush_size;
                        s.material = Some(mat.clone());
                        s
                    };

                    run_stroke(
                        &stroke,
                        &mut heights,
                        &mut blend,
                        dimension,
                        water_level_sample,
                        commit_per_stroke,
                        allows_tile_ref,
                        paint_material_fallback,
                        &mut on_texture,
                        &mut on_rejected_material,
                        &mut counters,
                    );
                }
            }
        }

        AiMapApplyResult::new(
            counters.total_height_samples,
            counters.total_material_strokes,
            counters.total_rejected_material_strokes,
            counters.total_collision_pixels,
            counters.operation_logs,
        )
    }

    pub fn choose_brush_size(radius: i32) -> i32 {
        if radius >= 7 {
            15
        } else if radius >= 4 {
            9
        } else if radius >= 2 {
            5
        } else if radius >= 1 {
            3
        } else {
            1
        }
    }

    pub fn generate_serpentine_path(
        min_x: i32,
        min_y: i32,
        max_x: i32,
        max_y: i32,
        step: i32,
    ) -> Vec<(i32, i32)> {
        let mut path = Vec::new();
        let step = step.max(1);
        let mut left_to_right = true;
        let mut y = min_y;

        while y <= max_y {
            if min_x == max_x {
                path.push((min_x, y));
            } else if left_to_right {
                path.push((min_x, y));
                path.push((max_x, y));
            } else {
                path.push((max_x, y));
                path.push((min_x, y));
            }
            left_to_right = !left_to_right;
            y += step;
        }

        if path.is_empty() {
            path.push((min_x, min_y));
        }
        path
    }

    pub fn generate_circle_serpentine_path(
        cx: i32,
        cy: i32,
        radius: i32,
        step: i32,
        dimension: usize,
    ) -> Vec<(i32, i32)> {
        let mut path = Vec::new();
        let step = step.max(1);
        let mut y_list = vec![cy];

        let mut y = cy - step;
        while y >= 0.max(cy - radius) {
            y_list.push(y);
            y -= step;
        }

        let mut y = cy + step;
        while y <= (dimension as i32 - 1).min(cy + radius) {
            y_list.push(y);
            y += step;
        }

        y_list.sort_unstable();

        let mut left_to_right = true;
        for y in y_list {
            let dy = (y - cy).abs();
            let dx = (0.max(radius * radius - dy * dy) as f32).sqrt().round() as i32;
            let min_x = 0.max(cx - dx);
            let max_x = (dimension as i32 - 1).min(cx + dx);

            if min_x <= max_x {
                if min_x == max_x {
                    path.push((min_x, y));
                } else if left_to_right {
                    path.push((min_x, y));
                    path.push((max_x, y));
                } else {
                    path.push((max_x, y));
                    path.push((min_x, y));
                }
                left_to_right = !left_to_right;
            }
        }

        if path.is_empty() {
            path.push((
                cx.clamp(0, dimension as i32 - 1),
                cy.clamp(0, dimension as i32 - 1),
            ));
        }
        path
    }

    pub fn get_center_height(
        heights: Option<&TerrainSession>,
        tile_x: i32,
        tile_y: i32,
        dimension: usize,
    ) -> i32 {
        let h = match heights {
            Some(h) => h,
            None => return 0,
        };
        let step = (h.vertex_size() - 1) as f32 / dimension as f32;
        let vx = ((tile_x as f32 + 0.5) * step).round() as usize;
        let vy = ((tile_y as f32 + 0.5) * step).round() as usize;
        let vx = vx.min(h.vertex_size() - 1);
        let vy = vy.min(h.vertex_size() - 1);
        h.heights()[vy * h.vertex_size() + vx] as i32
    }

    /// Canonical replayable `/brush` text (see [`crate::brush::BrushCommand`]).
    pub fn format_brush_command(stroke: &EditorBrushStroke) -> String {
        crate::brush::BrushCommand::format(stroke)
            .unwrap_or_else(|e| format!("/brush invalid ({e})"))
    }

    pub fn format_human_description(
        stroke: &EditorBrushStroke,
        result: &EditorBrushResult,
    ) -> String {
        let loc = match stroke.path.len() {
            0 => "無路徑".to_string(),
            1 => format!("({}, {})", stroke.path[0].0, stroke.path[0].1),
            2 => format!(
                "({}, {}) → ({}, {})",
                stroke.path[0].0, stroke.path[0].1, stroke.path[1].0, stroke.path[1].1
            ),
            n => format!("({}, {}) ... 共 {} 點", stroke.path[0].0, stroke.path[0].1, n),
        };

        match stroke.tool {
            EditorBrushTool::Height => match stroke.height_operation {
                EditorHeightBrush::Raise => format!(
                    "提高地形：位置 {}，大小 {}，強度 {}（變更 {} 個高度頂點）",
                    loc,
                    stroke.size,
                    if stroke.strength_level == 2 {
                        "強"
                    } else if stroke.strength_level == 0 {
                        "輕"
                    } else {
                        "中"
                    },
                    result.height_samples_changed
                ),
                EditorHeightBrush::Lower => format!(
                    "降低地形：位置 {}，大小 {}，強度 {}（變更 {} 個高度頂點）",
                    loc,
                    stroke.size,
                    if stroke.strength_level == 2 {
                        "強"
                    } else if stroke.strength_level == 0 {
                        "輕"
                    } else {
                        "中"
                    },
                    result.height_samples_changed
                ),
                EditorHeightBrush::Smooth => format!(
                    "平滑地形：位置 {}，大小 {}（變更 {} 個高度頂點）",
                    loc, stroke.size, result.height_samples_changed
                ),
                EditorHeightBrush::Flatten => format!(
                    "整平地形：位置 {}，大小 {}（變更 {} 個高度頂點）",
                    loc, stroke.size, result.height_samples_changed
                ),
                EditorHeightBrush::Roughen => format!(
                    "粗糙化地形：位置 {}，大小 {}（變更 {} 個高度頂點）",
                    loc, stroke.size, result.height_samples_changed
                ),
                EditorHeightBrush::Water => format!(
                    "水域下挖：位置 {}，大小 {}（變更 {} 個高度頂點，{} 個通行像素）",
                    loc,
                    stroke.size,
                    result.height_samples_changed,
                    result.collision_pixels_changed
                ),
            },
            EditorBrushTool::Collision => format!(
                "通行設定（{}）：路徑 {}，大小 {}（變更 {} 個通行像素）",
                if stroke.block { "阻擋" } else { "通行" },
                loc,
                stroke.size,
                result.collision_pixels_changed
            ),
            EditorBrushTool::Material => format!(
                "塗抹材質 [{}]：位置 {}，大小 {}（變更 {} 格材質）",
                stroke.material.as_deref().unwrap_or(""),
                loc,
                stroke.size,
                result.material_tiles_changed
            ),
        }
    }
}
