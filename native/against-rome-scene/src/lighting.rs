use crate::math::Vec3;
use std::collections::{HashMap, HashSet};

/// Pure 24-bit BI_RGB BMP decoder matching C# `MapLightingBitmap`.
#[derive(Clone, Debug)]
pub struct MapLightingBitmap {
    pub width: usize,
    pub height: usize,
    rgb: Vec<u8>,
}

impl MapLightingBitmap {
    pub fn parse(source: &[u8]) -> Result<Self, &'static str> {
        if source.len() < 54 || source[0] != b'B' || source[1] != b'M' {
            return Err("Missing or truncated lighting BMP header");
        }
        let read_u32 = |offset: usize| -> u32 {
            u32::from_le_bytes([
                source[offset],
                source[offset + 1],
                source[offset + 2],
                source[offset + 3],
            ])
        };
        let read_u16 = |offset: usize| -> u16 {
            u16::from_le_bytes([source[offset], source[offset + 1]])
        };

        let declared_size = read_u32(2) as usize;
        let offset = read_u32(10) as usize;
        if declared_size != source.len() || read_u32(6) != 0 {
            return Err("Invalid BMP size or reserved fields");
        }
        if read_u32(14) != 40 || read_u16(28) != 24 || read_u32(30) != 0 {
            return Err("Only 24-bit BITMAPINFOHEADER BI_RGB lighting BMPs are supported");
        }

        let width = read_u32(18) as i32;
        let signed_height = read_u32(22) as i32;
        if width <= 0 || signed_height == 0 || read_u16(26) != 1 {
            return Err("Invalid BMP dimensions or plane count");
        }

        let width = width as usize;
        let height = signed_height.unsigned_abs() as usize;
        let stride = ((width * 3 + 3) & !3) as usize;
        let extent = stride * height;
        let image_size = read_u32(34) as usize;

        if offset < 54
            || offset > source.len()
            || extent > source.len().saturating_sub(offset)
            || (image_size != 0 && image_size != extent)
        {
            return Err("Invalid BMP pixel extent");
        }

        let mut rgb = vec![0u8; width * height * 3];
        for y in 0..height {
            let stored_y = if signed_height < 0 {
                y
            } else {
                height - 1 - y
            };
            let row_start = offset + stored_y * stride;
            let row = &source[row_start..row_start + width * 3];
            for x in 0..width {
                let dest = (y * width + x) * 3;
                let src_x = x * 3;
                // BMP stores BGR; convert to RGB
                rgb[dest] = row[src_x + 2];
                rgb[dest + 1] = row[src_x + 1];
                rgb[dest + 2] = row[src_x];
            }
        }

        Ok(Self { width, height, rgb })
    }

    /// Returns unnormalized RGB color components in 0..255.
    pub fn color_at(&self, x: usize, y: usize) -> Vec3 {
        assert!(x < self.width && y < self.height, "out of bounds coordinates");
        let i = (y * self.width + x) * 3;
        Vec3::new(
            self.rgb[i] as f32,
            self.rgb[i + 1] as f32,
            self.rgb[i + 2] as f32,
        )
    }
}

/// 24x6 Day/Night lighting table matching C# `MapLightingDayNight`.
#[derive(Clone, Debug)]
pub struct MapLightingDayNight {
    bitmap: MapLightingBitmap,
}

impl MapLightingDayNight {
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        let bitmap = MapLightingBitmap::parse(bytes)?;
        if bitmap.width != 24 || bitmap.height != 6 {
            return Err("Expected a 24x6 daynight table");
        }
        Ok(Self { bitmap })
    }

    pub fn raw_color(&self, hour: usize, row: usize) -> Vec3 {
        self.bitmap.color_at(hour, row)
    }

    /// Replicates 0x49A350 native ambient evaluation from row 0 with 16.16 fixed-point floor weights.
    pub fn ambient_at(&self, hour: i32, minute: i32) -> Vec3 {
        let h = hour.clamp(0, 23) as usize;
        let m = minute.clamp(0, 59);

        let next_weight = (m * 65536) / 60;
        let current_weight = ((60 - m) * 65536) / 60;

        let current = self.raw_color(h, 0);
        let next = self.raw_color((h + 1) % 24, 0);

        let r = ((current.x as i32 * current_weight + next.x as i32 * next_weight) >> 16) as f32 / 256.0;
        let g = ((current.y as i32 * current_weight + next.y as i32 * next_weight) >> 16) as f32 / 256.0;
        let b = ((current.z as i32 * current_weight + next.z as i32 * next_weight) >> 16) as f32 / 256.0;

        Vec3::new(r, g, b)
    }
}

/// Native light calculation models matching C# `MapLightingModel` and `NativeLightCalculator`.
pub struct MapLightingModel;

impl MapLightingModel {
    /// Replicates 0x49FFE0 quadratic light falloff combined with ambient per-channel max, clamped to 1.0.
    pub fn apply_local_light(
        ambient: Vec3,
        light_color: Vec3,
        distance_squared: f32,
        radius_squared: f32,
    ) -> Vec3 {
        if !distance_squared.is_finite() || distance_squared < 0.0 {
            return ambient.min(Vec3::new(1.0, 1.0, 1.0));
        }
        if !radius_squared.is_finite() || radius_squared <= 0.0 {
            return ambient.min(Vec3::new(1.0, 1.0, 1.0));
        }
        if distance_squared >= radius_squared {
            return ambient.min(Vec3::new(1.0, 1.0, 1.0));
        }

        let falloff = 1.0 - (distance_squared / radius_squared);
        let candidate = light_color * falloff;

        let lit_x = ambient.x.max(candidate.x).min(1.0);
        let lit_y = ambient.y.max(candidate.y).min(1.0);
        let lit_z = ambient.z.max(candidate.z).min(1.0);

        Vec3::new(lit_x, lit_y, lit_z)
    }

    /// Replicates 0x49A490 sprite gain computation.
    pub fn sprite_gain(
        locally_lit: Vec3,
        cloud_and_terrain_shadow: f32,
        visibility_sample: u8,
    ) -> Vec3 {
        let shadow = cloud_and_terrain_shadow.clamp(0.0, 1.0);
        const NATIVE_SCALE: f32 = 0.0039062025; // 0x3B7FFF34
        let visibility = visibility_sample as f32 * NATIVE_SCALE;
        let factor = (shadow * 0.707 + 0.293) * visibility * visibility;
        locally_lit * factor
    }
}

/// Definition of a light source type from `lightdef.dau` matching C# `NativeLightDefinition`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeLightDefinition {
    pub index: i32,
    pub is_active: bool,
    pub color: Vec3,
    pub radius: f32,
    pub flicker_type: i32,
    pub flicker_param: f32,
    pub special_fx: i32,
    pub name: String,
}

impl Default for NativeLightDefinition {
    fn default() -> Self {
        Self::create_default(0)
    }
}

impl NativeLightDefinition {
    pub fn create_default(index: i32) -> Self {
        Self {
            index,
            is_active: true,
            color: Vec3::new(1.0, 1.0, 1.0),
            radius: 500.0,
            flicker_type: 0,
            flicker_param: 0.0,
            special_fx: 0,
            name: "DefaultLight".into(),
        }
    }

    pub fn radius_squared(&self) -> f32 {
        self.radius * self.radius
    }

    pub fn inv_radius_squared(&self) -> f32 {
        if self.radius > 0.0 {
            1.0 / (self.radius * self.radius)
        } else {
            0.0
        }
    }
}

/// Catalog of light definitions from `lightdef.dau` matching C# `NativeLightCatalog`.
#[derive(Clone, Debug, Default)]
pub struct NativeLightCatalog {
    definitions: HashMap<i32, NativeLightDefinition>,
}

impl NativeLightCatalog {
    pub fn new(definitions: impl IntoIterator<Item = NativeLightDefinition>) -> Self {
        let mut map = HashMap::new();
        for def in definitions {
            map.insert(def.index, def);
        }
        Self { definitions: map }
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn try_get_definition(&self, index: i32) -> Option<&NativeLightDefinition> {
        self.definitions.get(&index)
    }

    pub fn get_definition(&self, index: i32) -> NativeLightDefinition {
        self.definitions
            .get(&index)
            .cloned()
            .unwrap_or_else(|| NativeLightDefinition::create_default(index))
    }

    /// Parses `lightdef.dau` or `[LightDefault]` configuration text.
    /// Format: ;idx, activ, red, grn, blu, rad, type, typep, spefx, name
    pub fn parse(content: &str) -> Self {
        let mut defs = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                continue;
            }
            let line_body = if let Some(stripped) = line.strip_prefix(';') {
                stripped.trim()
            } else {
                line
            };
            if line_body.is_empty() {
                continue;
            }

            let cols: Vec<&str> = line_body.split(',').map(|s| s.trim()).collect();
            if cols.len() < 6 {
                continue;
            }

            let index = cols[0].parse::<i32>().unwrap_or(-1);
            if index < 0 {
                continue;
            }

            let is_active = cols[1].parse::<i32>().unwrap_or(0) != 0;
            let r = cols[2].parse::<f32>().unwrap_or(1.0);
            let g = cols[3].parse::<f32>().unwrap_or(1.0);
            let b = cols[4].parse::<f32>().unwrap_or(1.0);
            let radius = cols[5].parse::<f32>().unwrap_or(500.0);

            let flicker_type = if cols.len() > 6 {
                cols[6].parse::<i32>().unwrap_or(0)
            } else {
                0
            };
            let flicker_param = if cols.len() > 7 {
                cols[7].parse::<f32>().unwrap_or(0.0)
            } else {
                0.0
            };
            let special_fx = if cols.len() > 8 {
                cols[8].parse::<i32>().unwrap_or(0)
            } else {
                0
            };
            let name = if cols.len() > 9 {
                cols[9].to_string()
            } else {
                String::new()
            };

            defs.push(NativeLightDefinition {
                index,
                is_active,
                color: Vec3::new(r, g, b),
                radius,
                flicker_type,
                flicker_param,
                special_fx,
                name,
            });
        }
        Self::new(defs)
    }
}

/// Object lighting definition from `objdef.dau` matching C# `NativeObjectLightingDefinition`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeObjectLightingDefinition {
    pub type_id: i32,
    pub name: String,
    pub apt_index: i32,
    pub light_def_index: i32,
    pub light_height_offset: i32,
    pub apt_light_def_index: i32,
    pub apt_light_height_offset: i32,
}

/// Catalog of object lighting fields from `objdef.dau` matching C# `NativeObjectLightingCatalog`.
#[derive(Clone, Debug, Default)]
pub struct NativeObjectLightingCatalog {
    by_name: HashMap<String, NativeObjectLightingDefinition>,
    by_id: HashMap<i32, NativeObjectLightingDefinition>,
}

impl NativeObjectLightingCatalog {
    pub fn new(definitions: impl IntoIterator<Item = NativeObjectLightingDefinition>) -> Self {
        let mut by_name = HashMap::new();
        let mut by_id = HashMap::new();
        for def in definitions {
            by_name.insert(def.name.to_ascii_lowercase(), def.clone());
            by_id.insert(def.type_id, def);
        }
        Self { by_name, by_id }
    }

    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    pub fn try_get_definition(&self, name: &str) -> Option<&NativeObjectLightingDefinition> {
        self.by_name.get(&name.trim().to_ascii_lowercase())
    }

    pub fn try_get_definition_by_id(&self, type_id: i32) -> Option<&NativeObjectLightingDefinition> {
        self.by_id.get(&type_id)
    }

    /// Parses objdef.dau configuration lines.
    pub fn parse(text: &str) -> Self {
        let mut defs = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') || line.starts_with('[') {
                continue;
            }
            let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if cols.len() <= 67 {
                continue;
            }
            let type_id = match cols[0].parse::<i32>() {
                Ok(id) => id,
                Err(_) => continue,
            };

            let apt_index = if cols.len() > 14 {
                cols[14].parse::<i32>().unwrap_or(-1)
            } else {
                -1
            };

            let name = if cols.len() > 52 {
                cols[52].trim().to_string()
            } else {
                String::new()
            };
            if name.is_empty() {
                continue;
            }

            let lidef = cols[66].parse::<i32>().unwrap_or(-1);
            let lihei = cols[67].parse::<i32>().unwrap_or(0);

            let aptli = if cols.len() > 162 {
                cols[162].parse::<i32>().unwrap_or(-1)
            } else {
                -1
            };
            let aptlh = if cols.len() > 163 {
                cols[163].parse::<i32>().unwrap_or(0)
            } else {
                0
            };

            defs.push(NativeObjectLightingDefinition {
                type_id,
                name,
                apt_index,
                light_def_index: lidef,
                light_height_offset: lihei,
                apt_light_def_index: aptli,
                apt_light_height_offset: aptlh,
            });
        }
        Self::new(defs)
    }
}

/// Runtime instance of a local light source in the scene matching C# `NativeLightInstance`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeLightInstance {
    pub world_position: Vec3,
    pub color: Vec3,
    pub radius: f32,
    pub baseline_radius: f32,
    pub is_active: bool,
    pub is_saved: bool,
    pub saved_slot: i32,
    pub definition_index: Option<i32>,
}

impl NativeLightInstance {
    /// Creates a synthetic preview light from a `NativeLightDefinition`.
    pub fn create_synthetic(
        world_position: Vec3,
        def: &NativeLightDefinition,
        is_active: bool,
    ) -> Self {
        Self {
            world_position,
            color: def.color,
            radius: def.radius,
            baseline_radius: def.radius,
            is_active,
            is_saved: false,
            saved_slot: -1,
            definition_index: Some(def.index),
        }
    }

    /// Creates an immutable saved native light snapshot matching `NativeLightInstance.CreateSaved`.
    pub fn create_saved(
        slot: i32,
        native_position: Vec3,
        rgb: Vec3,
        effective_radius: f32,
        baseline_radius: f32,
        is_active: bool,
        definition_index: Option<i32>,
    ) -> Self {
        Self {
            world_position: native_position,
            color: rgb,
            radius: effective_radius,
            baseline_radius: if baseline_radius.is_nan() { effective_radius } else { baseline_radius },
            is_active,
            is_saved: true,
            saved_slot: slot,
            definition_index,
        }
    }

    pub fn radius_squared(&self) -> f32 {
        self.radius * self.radius
    }

    pub fn inv_radius_squared(&self) -> f32 {
        if self.radius > 0.0 {
            1.0 / (self.radius * self.radius)
        } else {
            0.0
        }
    }

    /// Light is renderable: active, finite XYZ, finite RGB, finite radius > 0.
    pub fn is_renderable(&self) -> bool {
        self.is_active
            && self.world_position.x.is_finite()
            && self.world_position.y.is_finite()
            && self.world_position.z.is_finite()
            && self.color.x.is_finite()
            && self.color.y.is_finite()
            && self.color.z.is_finite()
            && self.radius.is_finite()
            && self.radius > 0.0
    }
}

/// Explicit suppression key matching C# `SceneLightSuppressionKey` (source_file, object_index).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SceneLightSuppressionKey {
    pub source_file: String,
    pub object_index: i32,
}

impl SceneLightSuppressionKey {
    pub fn try_for(source_file: &str, object_index: i32) -> Option<Self> {
        if object_index == -1 {
            None
        } else {
            Some(Self {
                source_file: source_file.replace('\\', "/").to_ascii_uppercase(),
                object_index,
            })
        }
    }
}

/// Immutable collection of saved native lights matching C# `SavedSceneLightSet`.
#[derive(Clone, Debug, Default)]
pub struct SavedSceneLightSet {
    pub instances: Vec<NativeLightInstance>,
    pub suppressed_objects: HashSet<SceneLightSuppressionKey>,
}

impl SavedSceneLightSet {
    pub fn new(
        instances: Vec<NativeLightInstance>,
        suppressed_objects: HashSet<SceneLightSuppressionKey>,
    ) -> Result<Self, &'static str> {
        for light in &instances {
            if !light.is_saved {
                return Err("Only saved light instances are accepted in SavedSceneLightSet");
            }
        }
        Ok(Self {
            instances,
            suppressed_objects,
        })
    }

    pub fn has_data(&self) -> bool {
        !self.instances.is_empty() || !self.suppressed_objects.is_empty()
    }
}

/// High-level scene lighting context managing ambient and local light sources.
pub struct SceneLightingContext {
    pub day_night: Option<MapLightingDayNight>,
    pub light_catalog: Option<NativeLightCatalog>,
    pub object_lighting_catalog: Option<NativeObjectLightingCatalog>,
    pub apt_names: HashMap<i32, String>,
    pub apt_light_point_cache: HashMap<String, Vec<Vec3>>,
}

impl SceneLightingContext {
    pub fn new(
        day_night: Option<MapLightingDayNight>,
        light_catalog: Option<NativeLightCatalog>,
        object_lighting_catalog: Option<NativeObjectLightingCatalog>,
        apt_names: HashMap<i32, String>,
    ) -> Self {
        Self {
            day_night,
            light_catalog,
            object_lighting_catalog,
            apt_names,
            apt_light_point_cache: HashMap::new(),
        }
    }

    pub fn is_day_night_available(&self) -> bool {
        self.day_night.is_some()
    }

    pub fn is_light_catalog_available(&self) -> bool {
        self.light_catalog.is_some()
    }

    /// Matches C# GetAmbientColor: clamp finite hours to 0..24, wrap 24 to 0, and use noon for nonfinite hours.
    pub fn get_ambient_color(&self, game_hour: f32) -> Vec3 {
        let Some(ref dn) = self.day_night else {
            return Vec3::new(1.0, 1.0, 1.0);
        };
        let clamped = if game_hour.is_finite() {
            game_hour.clamp(0.0, 24.0) % 24.0
        } else {
            12.0
        };
        let hour = clamped.floor() as i32;
        let fraction = clamped - hour as f32;
        let minute = (fraction * 60.0).floor().clamp(0.0, 59.0) as i32;
        dn.ambient_at(hour, minute)
    }

    /// Collects active local light instances from scene objects.
    pub fn collect_scene_lights<F>(
        &mut self,
        objects: &[SceneLightSourceItem],
        sample_ground_height: F,
        suppressed: Option<&HashSet<SceneLightSuppressionKey>>,
    ) -> Vec<NativeLightInstance>
    where
        F: Fn(f32, f32) -> f32,
    {
        let mut result = Vec::new();
        let (Some(ref lights), Some(ref objs)) =
            (self.light_catalog.as_ref(), self.object_lighting_catalog.as_ref())
        else {
            return result;
        };

        for obj in objects {
            if let Some(suppressed_set) = suppressed {
                if let Some(key) = SceneLightSuppressionKey::try_for(&obj.source_file, obj.object_index) {
                    if suppressed_set.contains(&key) {
                        continue;
                    }
                }
            }

            let Some(def) = objs.try_get_definition(&obj.name) else {
                continue;
            };

            // 1. Building APT lights
            if def.apt_light_def_index >= 0 && def.apt_index >= 0 {
                if let Some(light_def) = lights.try_get_definition(def.apt_light_def_index) {
                    if light_def.is_active
                        && light_def.radius.is_finite()
                        && light_def.radius > 0.0
                        && light_def.color.x.is_finite()
                        && light_def.color.y.is_finite()
                        && light_def.color.z.is_finite()
                    {
                        if let Some(apt_name) = self.apt_names.get(&def.apt_index) {
                            if let Some(points) = self.apt_light_point_cache.get(apt_name) {
                                for delta in points {
                                    let x = obj.world_x + delta.x;
                                    let z = obj.world_z + delta.z;
                                    let ground = sample_ground_height(x, z);
                                    let world_pos = Vec3::new(
                                        x,
                                        ground + def.apt_light_height_offset as f32,
                                        z,
                                    );
                                    result.push(NativeLightInstance::create_synthetic(
                                        world_pos, light_def, true,
                                    ));
                                }
                            }
                        }
                    }
                }
            }

            // 2. Object independent lights
            if def.light_def_index >= 0 {
                if let Some(light_def) = lights.try_get_definition(def.light_def_index) {
                    if light_def.is_active
                        && light_def.radius.is_finite()
                        && light_def.radius > 0.0
                        && light_def.color.x.is_finite()
                        && light_def.color.y.is_finite()
                        && light_def.color.z.is_finite()
                    {
                        let world_pos = Vec3::new(
                            obj.world_x,
                            obj.world_y + def.light_height_offset as f32,
                            obj.world_z,
                        );
                        result.push(NativeLightInstance::create_synthetic(
                            world_pos, light_def, true,
                        ));
                    }
                }
            }
        }

        result
    }
}

/// Generic input item for scene lighting queries.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneLightSourceItem {
    pub name: String,
    pub world_x: f32,
    pub world_y: f32,
    pub world_z: f32,
    pub source_file: String,
    pub object_index: i32,
}

impl SceneLightSourceItem {
    pub fn new(
        name: impl Into<String>,
        world_x: f32,
        world_y: f32,
        world_z: f32,
        source_file: impl Into<String>,
        object_index: i32,
    ) -> Self {
        Self {
            name: name.into(),
            world_x,
            world_y,
            world_z,
            source_file: source_file.into(),
            object_index,
        }
    }
}

/// Maximum number of game lights supported simultaneously in GPU uniform arrays.
pub const MAXIMUM_GAME_LIGHTS: usize = 64;

/// Packed GPU uniform arrays for scene lighting matching `Map3DViewControl.SelectNearestSceneLights`.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedSceneLighting {
    pub light_positions: [f32; MAXIMUM_GAME_LIGHTS * 4],
    pub light_colors: [f32; MAXIMUM_GAME_LIGHTS * 4],
    pub selected_count: usize,
    pub diagnostics: SceneLightDiagnostics,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SceneLightDiagnostics {
    pub total_saved: usize,
    pub renderable_saved: usize,
    pub suppressed_objects: usize,
    pub synthesized_lights: usize,
    pub native_height_known: bool,
}

impl PreparedSceneLighting {
    /// Selects the nearest lights (up to 64) to `target_world` and packs them into uniform buffers.
    /// Saved lights use horizontal XZ distance squared; synthetic lights use 3D distance squared.
    pub fn prepare(
        all_lights: &[NativeLightInstance],
        target_world: Vec3,
        diagnostics: SceneLightDiagnostics,
    ) -> Self {
        let mut positions = [0.0f32; MAXIMUM_GAME_LIGHTS * 4];
        let mut colors = [0.0f32; MAXIMUM_GAME_LIGHTS * 4];

        let mut candidate_indices: Vec<usize> = (0..all_lights.len())
            .filter(|&i| {
                let light = &all_lights[i];
                !light.is_saved || light.is_renderable()
            })
            .collect();

        candidate_indices.sort_by(|&a, &b| {
            let dist_a = Self::light_distance(&all_lights[a], target_world);
            let dist_b = Self::light_distance(&all_lights[b], target_world);
            dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        let selected = candidate_indices.into_iter().take(MAXIMUM_GAME_LIGHTS);
        let mut count = 0;

        for idx in selected {
            let light = &all_lights[idx];
            let offset = count * 4;
            positions[offset] = light.world_position.x;
            positions[offset + 1] = light.world_position.y;
            positions[offset + 2] = light.world_position.z;
            positions[offset + 3] = light.radius;

            colors[offset] = light.color.x;
            colors[offset + 1] = light.color.y;
            colors[offset + 2] = light.color.z;
            colors[offset + 3] = if light.is_saved { 1.0 } else { 0.0 };

            count += 1;
        }

        Self {
            light_positions: positions,
            light_colors: colors,
            selected_count: count,
            diagnostics,
        }
    }

    /// Replicates the native 0x49FFE0 / GLSL GameLightingShader light calculation on the CPU.
    ///
    /// Evaluates ambient light plus up to 64 selected local lights with quadratic falloff,
    /// taking the per-channel maximum and clamping to 1.0.
    /// Saved lights (w == 1.0) use native XYZ coordinates and only take effect if `native_y < 1.0e37`.
    /// Preview lights (w == 0.0) compare against `tile_position * 256.0`.
    pub fn evaluate_lighting(
        &self,
        ambient: Vec3,
        game_lighting_enabled: bool,
        tile_position: Vec3,
        native_y: f32,
    ) -> Vec3 {
        if !game_lighting_enabled {
            return Vec3::new(1.0, 1.0, 1.0);
        }
        let mut lit = ambient;
        let preview_pos = tile_position * 256.0;
        let native_pos = Vec3::new(preview_pos.x, native_y, preview_pos.z);
        let native_known = native_y < 1.0e37;

        for i in 0..self.selected_count.min(MAXIMUM_GAME_LIGHTS) {
            let offset = i * 4;
            let radius = self.light_positions[offset + 3];
            let is_saved = self.light_colors[offset + 3] > 0.5;

            if is_saved && !native_known {
                continue;
            }

            let light_pos = Vec3::new(
                self.light_positions[offset],
                self.light_positions[offset + 1],
                self.light_positions[offset + 2],
            );
            let target_pos = if is_saved { native_pos } else { preview_pos };
            let delta = light_pos - target_pos;
            let d2 = delta.dot(delta);

            if radius > 0.0 && d2 < radius * radius {
                let falloff = 1.0 - d2 / (radius * radius);
                let light_color = Vec3::new(
                    self.light_colors[offset],
                    self.light_colors[offset + 1],
                    self.light_colors[offset + 2],
                );
                let candidate = light_color * falloff;
                lit = lit.max(candidate);
            }
        }

        lit.min(Vec3::new(1.0, 1.0, 1.0))
    }

    fn light_distance(light: &NativeLightInstance, target: Vec3) -> f32 {
        if light.is_saved {
            let dx = light.world_position.x - target.x;
            let dz = light.world_position.z - target.z;
            dx * dx + dz * dz
        } else {
            light.world_position.distance(target).powi(2)
        }
    }
}
