//! Authoring grid for Against Rome terrain transitions.
//!
//! Materials live on tile corners instead of tile centers, enabling native Against Rome
//! tiles to represent solid base, 2-material 4U transitions, or 3-material 4T junctions.

use crate::error::TextureError;
use crate::resolver::NativeTerrainMaterialResolver;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTerrainBakeIssue {
    pub tile_x: usize,
    pub tile_y: usize,
    pub corner_material_ids: [String; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTerrainCornerConflict {
    pub corner_x: usize,
    pub corner_y: usize,
    pub material_votes: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct NativeTerrainImportResult {
    pub map: TerrainBlendAuthoringMap,
    pub unresolved_tile_indices: Vec<usize>,
    pub corner_conflicts: Vec<NativeTerrainCornerConflict>,
}

#[derive(Debug, Clone)]
pub struct NativeTerrainBakeResult {
    pub textures: Vec<Option<String>>,
    pub issues: Vec<NativeTerrainBakeIssue>,
}

impl NativeTerrainBakeResult {
    pub fn is_complete(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Authoring grid where materials are placed on tile corners.
#[derive(Debug, Clone)]
pub struct TerrainBlendAuthoringMap {
    tile_dimension: usize,
    corner_dimension: usize,
    corner_materials: Vec<String>,
}

impl TerrainBlendAuthoringMap {
    /// Creates a new authoring map initialized with a uniform material.
    pub fn new(tile_dimension: usize, initial_material: &str) -> Result<Self, TextureError> {
        if tile_dimension == 0 {
            return Err(TextureError::InvalidFormat(
                "Tile dimension must be positive".to_string(),
            ));
        }
        if initial_material.trim().is_empty() {
            return Err(TextureError::EmptyMaterial);
        }

        let corner_dimension = tile_dimension + 1;
        let count = corner_dimension * corner_dimension;
        let corner_materials = vec![initial_material.trim().to_string(); count];

        Ok(Self {
            tile_dimension,
            corner_dimension,
            corner_materials,
        })
    }

    #[inline]
    pub fn tile_dimension(&self) -> usize {
        self.tile_dimension
    }

    #[inline]
    pub fn corner_dimension(&self) -> usize {
        self.corner_dimension
    }

    #[inline]
    pub fn corner_materials(&self) -> &[String] {
        &self.corner_materials
    }

    /// Imports an existing tile grid, voting on corners to reconstruct the authoring state.
    pub fn import(
        tile_dimension: usize,
        textures: &[String],
        catalog: &dyn NativeTerrainMaterialResolver,
        fallback_material: &str,
    ) -> Result<NativeTerrainImportResult, TextureError> {
        if tile_dimension == 0 || textures.len() != tile_dimension * tile_dimension {
            return Err(TextureError::DimensionMismatch {
                expected: tile_dimension * tile_dimension,
                actual: textures.len(),
            });
        }

        let mut map = Self::new(tile_dimension, fallback_material)?;
        let corner_count = map.corner_dimension * map.corner_dimension;
        let mut votes: Vec<HashMap<String, usize>> = vec![HashMap::new(); corner_count];
        let mut unresolved = Vec::new();

        for y in 0..tile_dimension {
            for x in 0..tile_dimension {
                let tile_index = y * tile_dimension + x;
                match catalog.try_resolve_native_corners(&textures[tile_index]) {
                    Some(corners) => {
                        // Order: TL, TR, BR, BL
                        // Corner (x, y)
                        add_vote(&mut votes, map.corner_dimension, x, y, &corners[0]);
                        // Corner (x + 1, y)
                        add_vote(&mut votes, map.corner_dimension, x + 1, y, &corners[1]);
                        // Corner (x + 1, y + 1)
                        add_vote(&mut votes, map.corner_dimension, x + 1, y + 1, &corners[2]);
                        // Corner (x, y + 1)
                        add_vote(&mut votes, map.corner_dimension, x, y + 1, &corners[3]);
                    }
                    None => {
                        unresolved.push(tile_index);
                    }
                }
            }
        }

        let mut conflicts = Vec::new();

        for y in 0..map.corner_dimension {
            for x in 0..map.corner_dimension {
                let corner_idx = y * map.corner_dimension + x;
                let corner_votes = &votes[corner_idx];
                if corner_votes.is_empty() {
                    continue;
                }

                // Pick highest vote, then alphabetical case-insensitive
                let mut sorted_votes: Vec<(&String, &usize)> = corner_votes.iter().collect();
                sorted_votes.sort_by(|a, b| {
                    b.1.cmp(a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
                });

                let selected = sorted_votes[0].0.clone();
                map.set_corner(x, y, &selected)?;

                if corner_votes.len() > 1 {
                    conflicts.push(NativeTerrainCornerConflict {
                        corner_x: x,
                        corner_y: y,
                        material_votes: corner_votes.clone(),
                    });
                }
            }
        }

        Ok(NativeTerrainImportResult {
            map,
            unresolved_tile_indices: unresolved,
            corner_conflicts: conflicts,
        })
    }

    /// Gets the corner material at (x, y).
    pub fn get_corner(&self, x: usize, y: usize) -> Result<&str, TextureError> {
        let index = self.corner_index(x, y)?;
        Ok(&self.corner_materials[index])
    }

    /// Sets the corner material at (x, y).
    pub fn set_corner(&mut self, x: usize, y: usize, material: &str) -> Result<(), TextureError> {
        let index = self.corner_index(x, y)?;
        let trimmed = material.trim();
        if trimmed.is_empty() {
            return Err(TextureError::EmptyMaterial);
        }
        self.corner_materials[index] = trimmed.to_string();
        Ok(())
    }

    /// Directly sets corner at linear index.
    pub fn set_corner_by_index(&mut self, index: usize, material: &str) -> Result<(), TextureError> {
        if index >= self.corner_materials.len() {
            return Err(TextureError::OutOfRange {
                x: index % self.corner_dimension,
                y: index / self.corner_dimension,
                dimension: self.corner_dimension,
            });
        }
        let trimmed = material.trim();
        if trimmed.is_empty() {
            return Err(TextureError::EmptyMaterial);
        }
        self.corner_materials[index] = trimmed.to_string();
        Ok(())
    }

    /// Paints a circle of corners centered at (center_x, center_y) with radius.
    /// Returns the linear indices of all modified corners.
    pub fn paint_circle(
        &mut self,
        center_x: f32,
        center_y: f32,
        radius: f32,
        material: &str,
    ) -> Result<Vec<usize>, TextureError> {
        if !center_x.is_finite() || !center_y.is_finite() {
            return Err(TextureError::InvalidRadius);
        }
        if !radius.is_finite() || radius < 0.0 {
            return Err(TextureError::InvalidRadius);
        }
        let trimmed = material.trim();
        if trimmed.is_empty() {
            return Err(TextureError::EmptyMaterial);
        }

        let radius_squared = radius * radius;
        let min_x = (center_x - radius).floor().max(0.0) as usize;
        let max_x = ((center_x + radius).ceil() as usize).min(self.corner_dimension - 1);
        let min_y = (center_y - radius).floor().max(0.0) as usize;
        let max_y = ((center_y + radius).ceil() as usize).min(self.corner_dimension - 1);

        let mut changed = Vec::new();

        for y in min_y..=max_y {
            let dy = y as f32 - center_y;
            for x in min_x..=max_x {
                let dx = x as f32 - center_x;
                if dx * dx + dy * dy <= radius_squared {
                    let index = y * self.corner_dimension + x;
                    if !self.corner_materials[index].eq_ignore_ascii_case(trimmed) {
                        self.corner_materials[index] = trimmed.to_string();
                        changed.push(index);
                    }
                }
            }
        }

        Ok(changed)
    }

    /// Bakes the current corner materials into native floor tiles using the resolver.
    pub fn bake(&self, catalog: &dyn NativeTerrainMaterialResolver) -> NativeTerrainBakeResult {
        let mut textures = Vec::with_capacity(self.tile_dimension * self.tile_dimension);
        let mut issues = Vec::new();

        for y in 0..self.tile_dimension {
            for x in 0..self.tile_dimension {
                let corners = [
                    self.corner_materials[y * self.corner_dimension + x].clone(),
                    self.corner_materials[y * self.corner_dimension + (x + 1)].clone(),
                    self.corner_materials[(y + 1) * self.corner_dimension + (x + 1)].clone(),
                    self.corner_materials[(y + 1) * self.corner_dimension + x].clone(),
                ];

                let corner_refs = [
                    corners[0].as_str(),
                    corners[1].as_str(),
                    corners[2].as_str(),
                    corners[3].as_str(),
                ];

                let resolved = catalog.resolve_native_tile(&corner_refs, x, y);
                if resolved.is_none() {
                    issues.push(NativeTerrainBakeIssue {
                        tile_x: x,
                        tile_y: y,
                        corner_material_ids: corners,
                    });
                }
                textures.push(resolved);
            }
        }

        NativeTerrainBakeResult { textures, issues }
    }

    fn corner_index(&self, x: usize, y: usize) -> Result<usize, TextureError> {
        if x >= self.corner_dimension || y >= self.corner_dimension {
            return Err(TextureError::OutOfRange {
                x,
                y,
                dimension: self.corner_dimension,
            });
        }
        Ok(y * self.corner_dimension + x)
    }
}

fn add_vote(
    votes: &mut [HashMap<String, usize>],
    corner_dim: usize,
    x: usize,
    y: usize,
    material: &str,
) {
    if x < corner_dim && y < corner_dim {
        let idx = y * corner_dim + x;
        *votes[idx].entry(material.trim().to_string()).or_insert(0) += 1;
    }
}
