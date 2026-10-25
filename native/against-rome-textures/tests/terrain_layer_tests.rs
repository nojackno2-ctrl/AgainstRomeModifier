use against_rome_textures::terrain_layer::{
    invalidate_height_caches, TerrainLayer, HEIGHT_DEPENDENT_CACHES,
};
use std::fs;

fn bmp24(w: usize, h: usize, rows_top_down: &[Vec<[u8; 3]>]) -> Vec<u8> {
    let stride = (w * 3 + 3) & !3;
    let mut b = vec![0u8; 54 + stride * h];
    b[0] = b'B';
    b[1] = b'M';
    b[10] = 54;
    b[14] = 40;
    b[18..22].copy_from_slice(&(w as i32).to_le_bytes());
    b[22..26].copy_from_slice(&(h as i32).to_le_bytes());
    b[26] = 1;
    b[28] = 24;
    for (y, row) in rows_top_down.iter().enumerate() {
        for (x, bgr) in row.iter().enumerate() {
            let o = 54 + (h - 1 - y) * stride + x * 3;
            b[o..o + 3].copy_from_slice(bgr);
        }
    }
    b
}

fn sample() -> Vec<u8> {
    bmp24(
        3,
        2,
        &[
            vec![[1, 10, 2], [3, 20, 4], [5, 30, 6]],
            vec![[7, 40, 8], [9, 50, 10], [11, 60, 12]],
        ],
    )
}

fn tmpdir(n: &str) -> std::path::PathBuf {
    let d = crate::temp_fixture::dir(&format!("ar_tl-{n}"));
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn decode_reads_top_down_order_and_green() {
    let l = TerrainLayer::decode(&sample()).unwrap();
    assert_eq!((l.width, l.height), (3, 2));
    assert_eq!(l.green, vec![10, 20, 30, 40, 50, 60]);
    assert_eq!(l.argb[0], 0xFF02_0A01);
}

#[test]
fn unchanged_pixels_preserved_changed_become_gray() {
    let src = sample();
    let l = TerrainLayer::decode(&src).unwrap();
    let same = l.encode_with_green(&l.green).unwrap();
    assert_eq!(same[54..], src[54..], "identity encode keeps pixel bytes exact");
    assert_eq!(u32::from_le_bytes(same[2..6].try_into().unwrap()) as usize, same.len());
    let mut v = l.green.clone();
    v[4] = 99;
    let l2 = TerrainLayer::decode(&l.encode_with_green(&v).unwrap()).unwrap();
    assert_eq!(l2.argb[4], 0xFF63_6363);
    for i in [0, 1, 2, 3, 5] {
        assert_eq!(l2.argb[i], l.argb[i]);
    }
}

#[test]
fn value_count_mismatch_and_bad_input_rejected() {
    let src = bmp24(1, 1, &[vec![[0, 0, 0]]]);
    let l = TerrainLayer::decode(&src).unwrap();
    assert!(l.encode_with_green(&[0, 0]).is_err());
    assert!(TerrainLayer::decode(&src[..40]).is_err());
    assert!(TerrainLayer::decode(&sample()[..60]).is_err());
}

#[test]
fn read_missing_is_none_and_write_roundtrips_atomically() {
    let d = tmpdir("rw");
    let p = d.join("boden.bmp");
    assert!(TerrainLayer::read(&p).unwrap().is_none());
    fs::write(&p, bmp24(2, 1, &[vec![[1, 2, 3], [4, 5, 6]]])).unwrap();
    let l = TerrainLayer::read(&p).unwrap().unwrap();
    l.write(&p, &[2, 77]).unwrap();
    let l2 = TerrainLayer::read(&p).unwrap().unwrap();
    assert_eq!(l2.green, vec![2, 77]);
    assert!(!d.join("boden.bmp.tmp").exists());
    fs::remove_dir_all(d).unwrap();
}

#[test]
fn invalidate_removes_only_height_caches_including_readonly() {
    let d = tmpdir("inv");
    fs::write(d.join("skydens.dat"), b"a").unwrap();
    fs::write(d.join("shadows.dat"), b"b").unwrap();
    let mut perms = fs::metadata(d.join("shadows.dat")).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(d.join("shadows.dat"), perms).unwrap();
    fs::write(d.join("boden.bmp"), b"keep").unwrap();
    let removed = invalidate_height_caches(&d).unwrap();
    assert_eq!(removed, vec!["skydens.dat", "shadows.dat"]);
    for n in HEIGHT_DEPENDENT_CACHES {
        assert!(!d.join(n).exists());
    }
    assert!(d.join("boden.bmp").exists());
    assert!(invalidate_height_caches(&d).unwrap().is_empty());
    fs::remove_dir_all(d).unwrap();
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
