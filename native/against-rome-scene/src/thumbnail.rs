//! Thumbnail geometry and reference-identity cache; rasterization is left to the UI.

use std::{collections::HashMap, sync::Arc};
use crate::minimap::Rect;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThumbnailSource {
    pub width: u32,
    pub height: u32,
}

impl ThumbnailSource {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThumbnailPlan {
    pub source: Arc<ThumbnailSource>,
    pub size: u32,
    pub destination: Rect,
}

#[derive(Default, Debug)]
pub struct SpriteThumbnailCache {
    size: u32,
    images: HashMap<usize, Arc<ThumbnailPlan>>,
}

impl SpriteThumbnailCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_size(&self) -> u32 {
        self.size
    }

    pub fn get(
        &mut self,
        source: Arc<ThumbnailSource>,
        size: u32,
    ) -> Result<Arc<ThumbnailPlan>, &'static str> {
        if size == 0 || size > i32::MAX as u32 || source.width == 0 || source.height == 0 {
            return Err("invalid thumbnail dimensions");
        }
        if self.size != size {
            self.clear();
            self.size = size;
        }
        // Key on raw Arc pointer address (reference identity), matching C# ReferenceEqualityComparer.
        let key = Arc::as_ptr(&source) as usize;
        Ok(self
            .images
            .entry(key)
            .or_insert_with(|| {
                let s = (size as f32 / source.width as f32).min(size as f32 / source.height as f32);
                let w = source.width as f32 * s;
                let h = source.height as f32 * s;
                Arc::new(ThumbnailPlan {
                    source: Arc::clone(&source),
                    size,
                    destination: Rect::new(
                        ((size as f32 - w) / 2.0).round_ties_even() as i32,
                        ((size as f32 - h) / 2.0).round_ties_even() as i32,
                        w.round_ties_even() as i32,
                        h.round_ties_even() as i32,
                    ),
                })
            })
            .clone())
    }

    pub fn clear(&mut self) {
        self.images.clear();
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}
