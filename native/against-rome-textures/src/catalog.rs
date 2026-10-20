//! High-level native sprite asset catalog, resolving map object definitions to ALR/APT sprites.
//!
//! Direct Rust counterpart to `src.MapEditor.Modules/NativeAssets/NativeSpriteCatalog.cs`.
//!
//! # Scope & Verification Limits
//! - Resolves map object names to cropped, ground-anchored straight-alpha ARGB sprites.
//! - Uses `against_rome_data::AlrDocument` and `against_rome_data::AptDocument` decoders.
//! - Direction row mapping follows template-matched game evidence (Angle 0 = row 14).
//! - Unsupported animation features:
//!   - Native runtime tick simulation, state-machine transitions (walk/attack/die),
//!     combat hit frames, and event triggers are NOT supported.
//!   - Parent-child hierarchy animation synchronization (e.g. rider + mount, catapult + crew)
//!     is NOT simulated.
//!   - Preview animations represent approximate cyclic loops based on frame count and cycle duration.
//! - Unsupported projection features:
//!   - 3D perspective distortion and dynamic camera tilt transformations are NOT supported;
//!     sprites are positioned via 2.5D orthographic/isometric ground contact anchors.
//!   - Vertex-shader GPU projection and dynamic particle blend modes are NOT supported.

use crate::error::TextureError;
use crate::sprite::{
    apt_finished_frame, decode_animation_frames, direction_for_angle, NativeSprite,
    NativeSpriteAnimation, NativeSpriteAtlas, DEFAULT_DIRECTION,
};
use against_rome_data::{AlrDocument, AptDocument};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// objdef row fields that select a native sprite: column 5 alrid, 8 alrml (second ALR layer),
/// 10 mltyp (layer type), 14 aptix, 17 palty and 52 name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeSpriteDefinition {
    pub type_id: i32,
    pub name: String,
    pub alr_id: i32,
    pub apt_index: i32,
    pub palette_type: i32,
    pub layer_alr_id: i32,
    pub layer_type: i32,
    pub animation_length_ms: i32,
    pub animation_add: i32,
    pub animation_mask: u32,
}

impl NativeSpriteDefinition {
    pub fn new(
        type_id: i32,
        name: impl Into<String>,
        alr_id: i32,
        apt_index: i32,
        palette_type: i32,
    ) -> Self {
        Self {
            type_id,
            name: name.into(),
            alr_id,
            apt_index,
            palette_type,
            layer_alr_id: -1,
            layer_type: -1,
            animation_length_ms: 0,
            animation_add: -1,
            animation_mask: 0,
        }
    }
}

/// Detached serialized animation state for one native object.
/// This is a snapshot, not a simulation of native ticks, construction progress, or parent animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NativeAnimationVisualState {
    pub alr_frame: i16,
    pub apt_axis0: i16,
    pub apt_axis1: i16,
    pub apt_axis2: i16,
    pub apt_axis3: i16,
}

impl NativeAnimationVisualState {
    pub const RECORD_SIZE: usize = 21;

    pub const fn new(
        alr_frame: i16,
        apt_axis0: i16,
        apt_axis1: i16,
        apt_axis2: i16,
        apt_axis3: i16,
    ) -> Self {
        Self {
            alr_frame,
            apt_axis0,
            apt_axis1,
            apt_axis2,
            apt_axis3,
        }
    }

    /// Reads exactly one unpacked anim.dat record; inactive or incorrectly sized
    /// records return None. Signed values are preserved without clamping or defaults.
    pub fn parse_record(record: &[u8]) -> Option<Self> {
        if record.len() != Self::RECORD_SIZE || record[0] == 0 {
            return None;
        }

        // Shipped EXE SHA256 6ac85239...: 48c250 maps disk +5 to runtime +8.
        // 4b53c0's frame=-1 fallback sign-extends that word with SAR 16.
        // 4b4790 -> 4cbf70 passes runtime +12/+16/+14/+18 (hex):
        // disk +13/+17/+15/+19. Disk +11 (runtime +10) is NOT axis0.
        let alr_frame = i16::from_le_bytes(record[5..7].try_into().ok()?);
        let apt_axis0 = i16::from_le_bytes(record[13..15].try_into().ok()?);
        let apt_axis1 = i16::from_le_bytes(record[17..19].try_into().ok()?);
        let apt_axis2 = i16::from_le_bytes(record[15..17].try_into().ok()?);
        let apt_axis3 = i16::from_le_bytes(record[19..21].try_into().ok()?);

        Some(Self {
            alr_frame,
            apt_axis0,
            apt_axis1,
            apt_axis2,
            apt_axis3,
        })
    }
}

/// Key for cached still sprites.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SpriteCacheKey {
    asset_name: String,
    variant: i32,
    direction: i32,
    state: Option<NativeAnimationVisualState>,
}

/// Key for cached preview animations.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct AnimationCacheKey {
    object_name: String,
    variant: i32,
    direction: i32,
    state: Option<NativeAnimationVisualState>,
}

struct DecodedLayer {
    width: i32,
    height: i32,
    pixels: Vec<u32>,
    anchor_x: i32,
    anchor_y: i32,
}

/// High-level catalog that maps game object definitions to decodable ALR/APT sprites and animations.
pub struct NativeSpriteCatalog {
    definitions: HashMap<String, NativeSpriteDefinition>,
    alr_names: HashMap<i32, String>,
    apt_names: HashMap<i32, String>,
    read_alr: Arc<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>,
    read_apt: Arc<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>,
    alr_documents: Mutex<HashMap<String, Option<Arc<AlrDocument>>>>,
    apt_documents: Mutex<HashMap<String, Option<Arc<AptDocument>>>>,
    sprites: Mutex<HashMap<SpriteCacheKey, Option<Arc<NativeSprite>>>>,
    animations: Mutex<HashMap<AnimationCacheKey, Option<Arc<NativeSpriteAnimation>>>>,
}

impl NativeSpriteCatalog {
    /// Constructs a catalog from in-memory text definitions and reader callbacks.
    pub fn from_text<F1, F2>(
        objdef_text: &str,
        alr_list_text: &str,
        apt_list_text: &str,
        read_alr: F1,
        read_apt: F2,
    ) -> Self
    where
        F1: Fn(&str) -> Option<Vec<u8>> + Send + Sync + 'static,
        F2: Fn(&str) -> Option<Vec<u8>> + Send + Sync + 'static,
    {
        Self {
            definitions: Self::parse_objdef(objdef_text),
            alr_names: Self::parse_name_list(alr_list_text),
            apt_names: Self::parse_name_list(apt_list_text),
            read_alr: Arc::new(read_alr),
            read_apt: Arc::new(read_apt),
            alr_documents: Mutex::new(HashMap::new()),
            apt_documents: Mutex::new(HashMap::new()),
            sprites: Mutex::new(HashMap::new()),
            animations: Mutex::new(HashMap::new()),
        }
    }

    /// Open the game's sprite sources read-only.
    /// Returns None when any required file is missing.
    pub fn open(game_path: impl AsRef<Path>) -> Option<Self> {
        let root = game_path.as_ref();
        let objdef_path = root
            .join("SYSTEM")
            .join("DATA_MP")
            .join("DEFAULTS")
            .join("objdef.dau");
        let alr_list_path = root.join("SYSTEM").join("cl_alr.ini");
        let apt_list_path = root.join("SYSTEM").join("cl_apt.ini");
        let alr_dat_path = root.join("alr.dat");
        let apt_dat_path = root.join("apt.dat");

        if !objdef_path.exists()
            || !alr_list_path.exists()
            || !apt_list_path.exists()
            || !alr_dat_path.exists()
            || !apt_dat_path.exists()
        {
            return None;
        }

        let objdef_text = read_game_text(&objdef_path).ok()?;
        let alr_list_text = read_game_text(&alr_list_path).ok()?;
        let apt_list_text = read_game_text(&apt_list_path).ok()?;

        let alr_file = File::open(alr_dat_path).ok()?;
        let apt_file = File::open(apt_dat_path).ok()?;

        let alr_archive = Arc::new(Mutex::new(zip::ZipArchive::new(alr_file).ok()?));
        let apt_archive = Arc::new(Mutex::new(zip::ZipArchive::new(apt_file).ok()?));

        let alr_reader = {
            let archive = Arc::clone(&alr_archive);
            move |name: &str| {
                let mut guard = archive.lock().ok()?;
                let target = format!("SYSTEM/DATA/ALR/{}", name);
                read_archive_entry(&mut guard, &target)
            }
        };

        let apt_reader = {
            let archive = Arc::clone(&apt_archive);
            move |name: &str| {
                let mut guard = archive.lock().ok()?;
                let target = format!("SYSTEM/DATA/APT/{}", name);
                read_archive_entry(&mut guard, &target)
            }
        };

        Some(Self::from_text(
            &objdef_text,
            &alr_list_text,
            &apt_list_text,
            alr_reader,
            apt_reader,
        ))
    }

    /// Number of loaded object definitions.
    pub fn definition_count(&self) -> usize {
        self.definitions.len()
    }

    /// Lookup an object definition by name (case-insensitive, trimmed).
    pub fn try_get_definition(&self, name: &str) -> Option<&NativeSpriteDefinition> {
        let key = name.trim().to_ascii_lowercase();
        self.definitions.get(&key)
    }

    /// Calculate the ALR still frame index for a given direction row.
    pub fn alr_still_frame(document: &AlrDocument, direction: i32) -> usize {
        if direction < 0 {
            return 0;
        }
        let frame = (direction as u64).saturating_mul((document.layout_columns as u64).max(1));
        if (frame as usize) < document.frames.len() {
            frame as usize
        } else {
            0
        }
    }

    /// Computes APT frame index from a 4-axis visual state.
    /// Returns None if out of bounds or invalid layout.
    pub fn try_apt_state_frame(
        layout: &[u32],
        frame_count: usize,
        state: &NativeAnimationVisualState,
    ) -> Option<usize> {
        if layout.len() != 4 || frame_count == 0 {
            return None;
        }
        let axes = [
            state.apt_axis0 as i32,
            state.apt_axis1 as i32,
            state.apt_axis2 as i32,
            state.apt_axis3 as i32,
        ];
        let mut index: i64 = 0;
        for i in 0..4 {
            if axes[i] < 0 || (axes[i] as u32) >= layout[i] {
                return None;
            }
            index = index
                .checked_mul(layout[i] as i64)?
                .checked_add(axes[i] as i64)?;
            if index > i32::MAX as i64 {
                return None;
            }
        }
        if (index as usize) >= frame_count {
            return None;
        }
        Some(index as usize)
    }

    /// Still sprite for a map object name; None when the object has no decodable native asset.
    /// A non-null `angle_degrees` selects the direction row via `direction_for_angle`.
    /// Explicit state selects the saved linear ALR frame or native-order APT axes, independently of angle.
    /// Invalid explicit indices return None without substituting a plausible legacy frame.
    pub fn get_sprite(
        &self,
        object_name: &str,
        team: i32,
        direction: i32,
        angle_degrees: Option<f32>,
        visual_state: Option<&NativeAnimationVisualState>,
    ) -> Option<Arc<NativeSprite>> {
        let definition = self.try_get_definition(object_name)?;

        // Trees and similar objects pair a base ALR (trunk) with an alrml layer (crown) drawn over it;
        // objects with only alrml (ground cover) use that layer alone.
        let primary = if definition.alr_id >= 0 {
            definition.alr_id
        } else {
            definition.layer_alr_id
        };
        let overlay = if definition.alr_id >= 0 {
            definition.layer_alr_id
        } else {
            -1
        };

        if primary >= 0 {
            if let Some(alr_name) = self.alr_names.get(&primary) {
                let document = self.load_alr(alr_name)?;
                let overlay_name = if overlay >= 0 && overlay != primary {
                    self.alr_names.get(&overlay)
                } else {
                    None
                };
                let overlay_document = overlay_name.and_then(|name| self.load_alr(name));

                if let Some(state) = visual_state {
                    if !valid_alr_frame(&document, state.alr_frame as i32)
                        || overlay_document
                            .as_ref()
                            .map_or(false, |doc| !valid_alr_frame(doc, state.alr_frame as i32))
                    {
                        return None;
                    }
                }

                // palty 1 objects carry per-team palette variants; others use the first palette.
                let variant = if definition.palette_type == 1 {
                    team.clamp(
                        0,
                        (document.palette_variant_count as i32).saturating_sub(1),
                    )
                } else {
                    0
                };

                let directions = (document.layout_rows as i32).max(1);
                let dir = if visual_state.is_some() {
                    0
                } else if let Some(angle) = angle_degrees {
                    direction_for_angle(angle, directions)
                } else {
                    ((direction % directions) + directions) % directions
                };

                let asset_name = if overlay_document.is_none() {
                    alr_name.clone()
                } else {
                    format!("{}+{}", alr_name, overlay_name.unwrap())
                };

                let key = SpriteCacheKey {
                    asset_name: asset_name.clone(),
                    variant,
                    direction: dir,
                    state: visual_state.copied(),
                };

                if let Ok(sprites_guard) = self.sprites.lock() {
                    if let Some(cached) = sprites_guard.get(&key) {
                        return cached.clone();
                    }
                }

                let linear_frame = visual_state.map(|s| s.alr_frame as i32);
                let decoded = decode_alr(
                    &asset_name,
                    &document,
                    overlay_document.as_deref(),
                    variant,
                    dir,
                    0,
                    linear_frame,
                );

                if let Ok(mut sprites_guard) = self.sprites.lock() {
                    sprites_guard.insert(key, decoded.clone());
                }
                return decoded;
            }
        }

        if definition.apt_index >= 0 {
            if let Some(apt_name) = self.apt_names.get(&definition.apt_index) {
                let document = self.load_apt(apt_name)?;
                let mut linear_frame = None;
                if let Some(state) = visual_state {
                    let index =
                        Self::try_apt_state_frame(&document.layout, document.frames.len(), state)?;
                    linear_frame = Some(index as i32);
                }

                let key = SpriteCacheKey {
                    asset_name: apt_name.clone(),
                    variant: 0,
                    direction: 0,
                    state: visual_state.copied(),
                };

                if let Ok(sprites_guard) = self.sprites.lock() {
                    if let Some(cached) = sprites_guard.get(&key) {
                        return cached.clone();
                    }
                }

                let decoded = decode_apt(apt_name, &document, 0, linear_frame);
                if let Ok(mut sprites_guard) = self.sprites.lock() {
                    sprites_guard.insert(key, decoded.clone());
                }
                return decoded;
            }
        }

        None
    }

    /// Cached preview sequence; static, missing-frame, or undecodable assets return None.
    /// Optional saved state seeds the same bank's preview cycle.
    pub fn get_animation(
        &self,
        object_name: &str,
        team: i32,
        angle_degrees: Option<f32>,
        visual_state: Option<&NativeAnimationVisualState>,
    ) -> Option<Arc<NativeSpriteAnimation>> {
        let definition = self.try_get_definition(object_name)?;

        let alr = if definition.alr_id >= 0 {
            self.alr_names
                .get(&definition.alr_id)
                .and_then(|name| self.load_alr(name))
        } else {
            None
        };

        let variant = if let Some(ref alr_doc) = alr {
            if definition.palette_type == 1 {
                team.clamp(
                    0,
                    (alr_doc.palette_variant_count as i32).saturating_sub(1),
                )
            } else {
                0
            }
        } else {
            0
        };

        if let (Some(state), Some(ref alr_doc)) = (visual_state, &alr) {
            if !valid_alr_frame(alr_doc, state.alr_frame as i32) {
                return None;
            }
        }

        let direction = if visual_state.is_some() || alr.is_none() {
            0
        } else {
            let rows = (alr.as_ref().unwrap().layout_rows as i32).max(1);
            if let Some(angle) = angle_degrees {
                direction_for_angle(angle, rows)
            } else {
                DEFAULT_DIRECTION % rows
            }
        };

        let key = AnimationCacheKey {
            object_name: definition.name.clone(),
            variant,
            direction,
            state: visual_state.copied(),
        };

        if let Ok(anim_guard) = self.animations.lock() {
            if let Some(cached) = anim_guard.get(&key) {
                return cached.clone();
            }
        }

        let mut animation = None;

        if let Some(ref alr_doc) = alr {
            let unit = definition.layer_type != 2
                && (definition.palette_type == 1
                    || (definition.animation_add >= 0 && definition.animation_mask != 0));
            let count = alr_doc.layout_columns as usize;
            let bank = if visual_state.is_none() || count == 0 {
                0
            } else {
                (visual_state.unwrap().alr_frame as usize / count) * count
            };
            let step = if visual_state.is_none() || count == 0 {
                0
            } else {
                visual_state.unwrap().alr_frame as usize % count
            };

            let valid_range = if visual_state.is_none() {
                (count as u64) * ((alr_doc.layout_rows as u64).max(1))
                    <= alr_doc.frames.len() as u64
            } else {
                (bank as u64) + (count as u64) <= alr_doc.frames.len() as u64
            };

            if unit && definition.animation_length_ms > 1 && count > 1 && valid_range {
                let overlay = if definition.layer_alr_id >= 0
                    && definition.layer_alr_id != definition.alr_id
                {
                    self.alr_names
                        .get(&definition.layer_alr_id)
                        .and_then(|name| self.load_alr(name))
                } else {
                    None
                };

                let alr_name = self.alr_names.get(&definition.alr_id).unwrap();
                let still = self.get_sprite(
                    object_name,
                    team,
                    DEFAULT_DIRECTION,
                    angle_degrees,
                    visual_state,
                );

                animation = decode_animation_frames(
                    count,
                    definition.animation_length_ms as u32,
                    still,
                    |frame| {
                        let linear = if visual_state.is_none() {
                            None
                        } else {
                            Some((bank + ((step + frame) % count)) as i32)
                        };
                        decode_alr(
                            alr_name,
                            alr_doc,
                            overlay.as_deref(),
                            variant,
                            direction,
                            frame as i32,
                            linear,
                        )
                    },
                )
                .map(Arc::new);
            }
        } else if definition.alr_id < 0
            && definition.layer_alr_id < 0
            && definition.apt_index >= 0
        {
            if let Some(apt_name) = self.apt_names.get(&definition.apt_index) {
                if let Some(apt) = self.load_apt(apt_name) {
                    if apt.layout.len() == 4 {
                        let count = apt.layout[3] as usize;
                        let mut step = 0;
                        let first_opt = if let Some(state) = visual_state {
                            if let Some(current) =
                                Self::try_apt_state_frame(&apt.layout, apt.frames.len(), state)
                            {
                                step = state.apt_axis3 as usize;
                                Some(current.saturating_sub(step))
                            } else {
                                None
                            }
                        } else {
                            Some(apt_finished_frame(&apt.layout, apt.frames.len()))
                        };

                        if let Some(first) = first_opt {
                            let cycle = if definition.animation_length_ms > 1 {
                                definition.animation_length_ms as u32
                            } else if count == 50 {
                                4500
                            } else {
                                3000
                            };

                            if count > 1
                                && (first as u64) + (count as u64) <= apt.frames.len() as u64
                            {
                                let still =
                                    self.get_sprite(object_name, team, 0, None, visual_state);
                                animation = decode_animation_frames(count, cycle, still, |frame| {
                                    let linear = if visual_state.is_none() {
                                        None
                                    } else {
                                        Some((first + ((step + frame) % count)) as i32)
                                    };
                                    decode_apt(apt_name, &apt, frame as i32, linear)
                                })
                                .map(Arc::new);
                            }
                        }
                    }
                }
            }
        }

        if let Ok(mut anim_guard) = self.animations.lock() {
            anim_guard.insert(key, animation.clone());
        }
        animation
    }

    /// Pack a list of sprites into an ARGB texture atlas.
    pub fn build_atlas(
        sprites: &[Arc<NativeSprite>],
        max_size: i32,
    ) -> Result<NativeSpriteAtlas, TextureError> {
        NativeSpriteAtlas::pack(sprites, max_size)
    }

    /// Pack sprites and animations atomically into an ARGB texture atlas.
    pub fn build_atlas_with_animations(
        sprites: &[Arc<NativeSprite>],
        animations: &[NativeSpriteAnimation],
        max_size: i32,
    ) -> Result<NativeSpriteAtlas, TextureError> {
        NativeSpriteAtlas::pack_animations(sprites, animations, max_size)
    }

    fn load_alr(&self, name: &str) -> Option<Arc<AlrDocument>> {
        if let Ok(guard) = self.alr_documents.lock() {
            if let Some(cached) = guard.get(name) {
                return cached.clone();
            }
        }
        let doc = (self.read_alr)(name)
            .and_then(|bytes| AlrDocument::parse(&bytes).ok())
            .map(Arc::new);
        if let Ok(mut guard) = self.alr_documents.lock() {
            guard.insert(name.to_string(), doc.clone());
        }
        doc
    }

    fn load_apt(&self, name: &str) -> Option<Arc<AptDocument>> {
        if let Ok(guard) = self.apt_documents.lock() {
            if let Some(cached) = guard.get(name) {
                return cached.clone();
            }
        }
        let doc = (self.read_apt)(name)
            .and_then(|bytes| AptDocument::parse(&bytes).ok())
            .map(Arc::new);
        if let Ok(mut guard) = self.apt_documents.lock() {
            guard.insert(name.to_string(), doc.clone());
        }
        doc
    }

    /// Parse objdef.dau table into map of definitions.
    pub fn parse_objdef(text: &str) -> HashMap<String, NativeSpriteDefinition> {
        let mut result = HashMap::new();
        for line in text.lines() {
            let trimmed = line.trim_start();
            if line.len() < 100 || trimmed.starts_with(';') || trimmed.starts_with('[') {
                continue;
            }
            let columns: Vec<&str> = line.split(',').collect();
            if columns.len() <= 52 {
                continue;
            }
            let id: i32 = match columns[0].trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let alr: i32 = match columns[5].trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let apt: i32 = match columns[14].trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let palette: i32 = match columns[17].trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let layer: i32 = columns[8].trim().parse().unwrap_or(-1);
            let layer_type: i32 = columns[10].trim().parse().unwrap_or(-1);
            let cycle: i32 = columns[2].trim().parse().unwrap_or(0);
            let animation_add: i32 = columns[6].trim().parse().unwrap_or(-1);
            let animation_mask: u32 = if columns.len() > 45 {
                u32::from_str_radix(columns[45].trim(), 16).unwrap_or(0)
            } else {
                0
            };
            let name = columns[52].trim();
            if !name.is_empty() {
                let key = name.to_ascii_lowercase();
                result.entry(key).or_insert_with(|| NativeSpriteDefinition {
                    type_id: id,
                    name: name.to_string(),
                    alr_id: alr,
                    apt_index: apt,
                    palette_type: palette,
                    layer_alr_id: layer,
                    layer_type,
                    animation_length_ms: cycle,
                    animation_add,
                    animation_mask,
                });
            }
        }
        result
    }

    /// Parse cl_alr.ini or cl_apt.ini comma-separated index-name mapping.
    pub fn parse_name_list(text: &str) -> HashMap<i32, String> {
        let mut result = HashMap::new();
        for line in text.lines() {
            let columns: Vec<&str> = line.trim().split(',').collect();
            if columns.len() > 1 {
                if let Ok(id) = columns[0].trim().parse::<i32>() {
                    let name = columns[1].trim();
                    if !name.is_empty() {
                        result.entry(id).or_insert_with(|| name.to_string());
                    }
                }
            }
        }
        result
    }
}

fn valid_alr_frame(document: &AlrDocument, frame: i32) -> bool {
    frame >= 0 && (frame as usize) < document.frames.len()
}

fn decode_alr_layer(
    document: &AlrDocument,
    variant: i32,
    direction: i32,
    frame_offset: i32,
    linear_frame: Option<i32>,
) -> Option<DecodedLayer> {
    let rows = (document.layout_rows as i32).max(1);
    let dir_row = ((direction % rows) + rows) % rows;
    let index = if let Some(linear) = linear_frame {
        if linear < 0 {
            return None;
        }
        linear as usize
    } else {
        NativeSpriteCatalog::alr_still_frame(document, dir_row)
            .saturating_add(frame_offset.max(0) as usize)
    };

    if index >= document.frames.len() {
        return None;
    }

    let info = &document.frames[index].info;
    let variant_clamped =
        (variant.max(0) as usize).min(document.palette_variant_count.saturating_sub(1));
    let pixels = document.decode_frame_argb(index, variant_clamped).ok()?;
    let anchor_x = (document.anchor_width as i32) / 2 - info.offset_x;
    let anchor_y = (document.anchor_height as i32) / 2 - info.offset_y;

    Some(DecodedLayer {
        width: info.width as i32,
        height: info.height as i32,
        pixels,
        anchor_x,
        anchor_y,
    })
}

fn decode_alr(
    name: &str,
    document: &AlrDocument,
    overlay: Option<&AlrDocument>,
    variant: i32,
    direction: i32,
    frame_offset: i32,
    linear_frame: Option<i32>,
) -> Option<Arc<NativeSprite>> {
    if let Some(explicit_frame) = linear_frame {
        if !valid_alr_frame(document, explicit_frame)
            || (overlay.is_some() && !valid_alr_frame(overlay.unwrap(), explicit_frame))
        {
            return None;
        }
    }

    let bottom_layer =
        decode_alr_layer(document, variant, direction, frame_offset, linear_frame)?;
    let top_layer = if let Some(overlay_doc) = overlay {
        let overlay_offset = if overlay_doc.layout_columns == document.layout_columns
            && overlay_doc.layout_rows == document.layout_rows
        {
            frame_offset
        } else {
            0
        };
        decode_alr_layer(overlay_doc, variant, direction, overlay_offset, linear_frame)
    } else {
        None
    };

    if let Some(top) = top_layer {
        let left = (-bottom_layer.anchor_x).min(-top.anchor_x);
        let top_bound = (-bottom_layer.anchor_y).min(-top.anchor_y);
        let right = (bottom_layer.width - bottom_layer.anchor_x).max(top.width - top.anchor_x);
        let bottom_bound =
            (bottom_layer.height - bottom_layer.anchor_y).max(top.height - top.anchor_y);
        let width = right - left;
        let height = bottom_bound - top_bound;
        if width <= 0 || height <= 0 {
            return None;
        }

        let mut pixels = vec![0u32; (width as usize) * (height as usize)];
        for layer in [&bottom_layer, &top] {
            let ox = -layer.anchor_x - left;
            let oy = -layer.anchor_y - top_bound;
            for y in 0..layer.height {
                for x in 0..layer.width {
                    let color = layer.pixels[(y as usize) * (layer.width as usize) + (x as usize)];
                    if (color >> 24) != 0 {
                        let dst_idx = ((oy + y) as usize) * (width as usize) + ((ox + x) as usize);
                        pixels[dst_idx] = color;
                    }
                }
            }
        }

        NativeSprite::crop(name, width, height, &pixels, -left, -top_bound).map(Arc::new)
    } else {
        let base_name = name.split('+').next().unwrap_or(name);
        NativeSprite::crop(
            base_name,
            bottom_layer.width,
            bottom_layer.height,
            &bottom_layer.pixels,
            bottom_layer.anchor_x,
            bottom_layer.anchor_y,
        )
        .map(Arc::new)
    }
}

fn decode_apt(
    name: &str,
    document: &AptDocument,
    frame_offset: i32,
    linear_frame: Option<i32>,
) -> Option<Arc<NativeSprite>> {
    let index = if let Some(linear) = linear_frame {
        if linear < 0 {
            return None;
        }
        linear as usize
    } else {
        let finished = apt_finished_frame(&document.layout, document.frames.len());
        (finished as i32)
            .checked_add(frame_offset)
            .filter(|&i| i >= 0)? as usize
    };

    if index >= document.frames.len() {
        return None;
    }

    let image = document.decode_frame(index, 0).ok()?;
    NativeSprite::crop(
        name,
        image.width as i32,
        image.height as i32,
        &image.argb_pixels,
        document.anchor_x,
        document.anchor_y,
    )
    .map(Arc::new)
}

fn read_game_text(path: &Path) -> Result<String, TextureError> {
    let bytes = std::fs::read(path).map_err(TextureError::Io)?;
    let decompressed = crate::lzss::decompress_if_pfil(&bytes)?;
    let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(&decompressed);
    Ok(cow.into_owned())
}

fn read_archive_entry<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    target_path: &str,
) -> Option<Vec<u8>> {
    if let Ok(mut file) = archive.by_name(target_path) {
        let mut buf = Vec::new();
        if file.read_to_end(&mut buf).is_ok() {
            return Some(buf);
        }
    }
    let alt_path = target_path.replace('/', "\\");
    if let Ok(mut file) = archive.by_name(&alt_path) {
        let mut buf = Vec::new();
        if file.read_to_end(&mut buf).is_ok() {
            return Some(buf);
        }
    }
    let normalized = target_path.replace('\\', "/").to_ascii_lowercase();
    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            let entry_name = file.name().replace('\\', "/").to_ascii_lowercase();
            if entry_name == normalized {
                let mut buf = Vec::new();
                if file.read_to_end(&mut buf).is_ok() {
                    return Some(buf);
                }
            }
        }
    }
    None
}
