use against_rome_scene::{Rect, SpriteThumbnailCache, ThumbnailSource};
use std::sync::Arc;

#[test]
fn thumbnail_aspect_ratio_scaling_and_centering() {
    let mut cache = SpriteThumbnailCache::new();

    // Square 100x100 into 64x64
    let square = Arc::new(ThumbnailSource::new(100, 100));
    let plan = cache.get(square, 64).unwrap();
    assert_eq!(Rect::new(0, 0, 64, 64), plan.destination);

    // Wide 200x100 (2:1) into 64x64 -> width 64, height 32, centered at y = (64 - 32) / 2 = 16
    let wide = Arc::new(ThumbnailSource::new(200, 100));
    let plan_wide = cache.get(wide, 64).unwrap();
    assert_eq!(Rect::new(0, 16, 64, 32), plan_wide.destination);

    // Tall 100x200 (1:2) into 64x64 -> width 32, height 64, centered at x = (64 - 32) / 2 = 16
    let tall = Arc::new(ThumbnailSource::new(100, 200));
    let plan_tall = cache.get(tall, 64).unwrap();
    assert_eq!(Rect::new(16, 0, 32, 64), plan_tall.destination);
}

#[test]
fn thumbnail_cache_reference_identity() {
    let mut cache = SpriteThumbnailCache::new();
    let source = Arc::new(ThumbnailSource::new(64, 64));

    let plan1 = cache.get(Arc::clone(&source), 32).unwrap();
    let plan2 = cache.get(Arc::clone(&source), 32).unwrap();

    // Must return the exact same Arc (cached)
    assert!(Arc::ptr_eq(&plan1, &plan2));
    assert_eq!(1, cache.len());
}

#[test]
fn thumbnail_cache_size_invalidation() {
    let mut cache = SpriteThumbnailCache::new();
    let source = Arc::new(ThumbnailSource::new(64, 64));

    let plan32 = cache.get(Arc::clone(&source), 32).unwrap();
    assert_eq!(32, cache.current_size());
    assert_eq!(32, plan32.size);

    // Requesting size 64 should invalidate the cache
    let plan64 = cache.get(Arc::clone(&source), 64).unwrap();
    assert_eq!(64, cache.current_size());
    assert_eq!(64, plan64.size);
    assert_eq!(1, cache.len());
}

#[test]
fn thumbnail_cache_dimension_validation() {
    let mut cache = SpriteThumbnailCache::new();
    let valid = Arc::new(ThumbnailSource::new(32, 32));

    // Zero size
    assert!(cache.get(Arc::clone(&valid), 0).is_err());

    // Zero width or height source
    let zero_w = Arc::new(ThumbnailSource::new(0, 32));
    assert!(cache.get(zero_w, 32).is_err());

    let zero_h = Arc::new(ThumbnailSource::new(32, 0));
    assert!(cache.get(zero_h, 32).is_err());
}

#[test]
fn thumbnail_clear_and_is_empty() {
    let mut cache = SpriteThumbnailCache::new();
    assert!(cache.is_empty());

    let source = Arc::new(ThumbnailSource::new(64, 64));
    cache.get(source, 32).unwrap();
    assert!(!cache.is_empty());
    assert_eq!(1, cache.len());

    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(0, cache.len());
}
