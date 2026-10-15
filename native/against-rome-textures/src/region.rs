//! Region shape planning algorithms for terrain editing (Rectangle, Connected fill, Polyline).

use crate::error::TextureError;
use std::collections::{HashSet, VecDeque};

pub struct TerrainRegionPlanner;

impl TerrainRegionPlanner {
    /// Generates all tile coordinates within the bounding box defined by two opposite corners `a` and `b`.
    pub fn rectangle(
        dimension: usize,
        a: (usize, usize),
        b: (usize, usize),
    ) -> Result<Vec<(usize, usize)>, TextureError> {
        Self::validate(dimension, a)?;
        Self::validate(dimension, b)?;

        let min_x = a.0.min(b.0);
        let max_x = a.0.max(b.0);
        let min_y = a.1.min(b.1);
        let max_y = a.1.max(b.1);

        let mut cells = Vec::new();
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                cells.push((x, y));
            }
        }
        Ok(cells)
    }

    /// Performs connected 4-way flood-fill starting from `seed`, matching on identical material ID.
    pub fn connected(
        dimension: usize,
        materials: &[String],
        seed: (usize, usize),
    ) -> Result<Vec<(usize, usize)>, TextureError> {
        Self::validate(dimension, seed)?;
        if materials.len() != dimension * dimension {
            return Err(TextureError::DimensionMismatch {
                expected: dimension * dimension,
                actual: materials.len(),
            });
        }

        let seed_idx = seed.1 * dimension + seed.0;
        let target_material = materials[seed_idx].clone();

        let mut visited = vec![false; materials.len()];
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        visited[seed_idx] = true;
        queue.push_back(seed);

        let dirs: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

        while let Some((cx, cy)) = queue.pop_front() {
            result.push((cx, cy));

            for (dx, dy) in dirs {
                let nx = cx as isize + dx;
                let ny = cy as isize + dy;
                if nx >= 0 && nx < dimension as isize && ny >= 0 && ny < dimension as isize {
                    let n_idx = (ny as usize) * dimension + (nx as usize);
                    if !visited[n_idx]
                        && materials[n_idx].eq_ignore_ascii_case(&target_material)
                    {
                        visited[n_idx] = true;
                        queue.push_back((nx as usize, ny as usize));
                    }
                }
            }
        }

        Ok(result)
    }

    /// Generates connected polyline cells with a specified brush width using the Bresenham line algorithm.
    pub fn polyline(
        dimension: usize,
        vertices: &[(usize, usize)],
        width: usize,
    ) -> Result<Vec<(usize, usize)>, TextureError> {
        if vertices.len() < 2 {
            return Err(TextureError::InvalidFormat(
                "At least two vertices are required for polyline".to_string(),
            ));
        }
        if width < 1 || width > dimension {
            return Err(TextureError::InvalidFormat(format!(
                "Invalid width {} for dimension {}",
                width, dimension
            )));
        }

        for &v in vertices {
            Self::validate(dimension, v)?;
        }

        let mut cells = HashSet::new();

        let lower = (width - 1) / 2;
        let upper = width / 2;

        let mut stamp = |x: usize, y: usize| {
            let min_y = if y >= lower { y - lower } else { 0 };
            let max_y = (y + upper).min(dimension - 1);
            let min_x = if x >= lower { x - lower } else { 0 };
            let max_x = (x + upper).min(dimension - 1);

            for yy in min_y..=max_y {
                for xx in min_x..=max_x {
                    cells.insert((xx, yy));
                }
            }
        };

        for i in 1..vertices.len() {
            let a = vertices[i - 1];
            let b = vertices[i];

            let mut x = a.0 as isize;
            let mut y = a.1 as isize;
            let target_x = b.0 as isize;
            let target_y = b.1 as isize;

            let dx = (target_x - x).abs();
            let dy = (target_y - y).abs();
            let sx = if x < target_x { 1 } else { -1 };
            let sy = if y < target_y { 1 } else { -1 };
            let mut error = dx - dy;

            loop {
                stamp(x as usize, y as usize);
                if x == target_x && y == target_y {
                    break;
                }
                let twice = 2 * error;
                if twice > -dy {
                    error -= dy;
                    x += sx;
                    stamp(x as usize, y as usize);
                }
                if twice < dx {
                    error += dx;
                    y += sy;
                }
            }
        }

        let mut result: Vec<(usize, usize)> = cells.into_iter().collect();
        result.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        Ok(result)
    }

    fn validate(dimension: usize, pt: (usize, usize)) -> Result<(), TextureError> {
        if pt.0 >= dimension || pt.1 >= dimension {
            Err(TextureError::OutOfRange {
                x: pt.0,
                y: pt.1,
                dimension,
            })
        } else {
            Ok(())
        }
    }
}
