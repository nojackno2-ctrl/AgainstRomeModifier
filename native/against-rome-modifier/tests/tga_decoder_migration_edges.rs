//! Extra TgaDecoder.cs parity edges. Expected values were produced by running the real C#
//! `TgaDecoder.Decode` (net10.0, throwaway project linking the source file) on these exact inputs;
//! the source SHA-256 guard fails if the C# decoder changes so the table must be regenerated.
use against_rome_modifier::tga;


fn header(kind: u8, w: u16, h: u16, depth: u8, desc: u8, id_len: u8) -> Vec<u8> {
    let mut b = vec![0u8; 18];
    b[0] = id_len;
    b[2] = kind;
    b[12..14].copy_from_slice(&w.to_le_bytes());
    b[14..16].copy_from_slice(&h.to_le_bytes());
    b[16] = depth;
    b[17] = desc;
    b
}
fn pal(w: u16, h: u16, desc: u8, map_len: u16, id_len: u8, map_type: u8, entry: u8, depth: u8) -> Vec<u8> {
    let mut b = header(1, w, h, depth, desc, id_len);
    b[1] = map_type;
    b[5..7].copy_from_slice(&map_len.to_le_bytes());
    b[7] = entry;
    b
}
fn p(w: u16, h: u16, desc: u8, map_len: u16) -> Vec<u8> {
    pal(w, h, desc, map_len, 0, 1, 24, 8)
}
fn with(mut b: Vec<u8>, tail: &[u8]) -> Vec<u8> {
    b.extend_from_slice(tail);
    b
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}
fn check(name: &str, input: Vec<u8>, expected: Option<(usize, usize, &str)>) {
    match (tga::decode(&input), expected) {
        (None, None) => {}
        (Some(i), Some((w, h, px))) => {
            assert_eq!((i.width, i.height, hex(&i.bgra)), (w, h, px.to_string()), "{name}")
        }
        (got, want) => panic!("{name}: rust={:?} csharp={want:?}", got.map(|i| (i.width, i.height))),
    }
}

#[test]
fn paletted_edges_match_csharp() {
    let m3 = [0, 0, 0, 9, 8, 7, 1, 2, 3];
    check("bottom_up", with(p(2, 2, 0, 3), &[&m3[..], &[0, 1, 2, 1]].concat()), Some((2, 2, "010203FF090807FF00000000090807FF")));
    check("top_down", with(p(2, 2, 0x20, 3), &[&m3[..], &[0, 1, 2, 1]].concat()), Some((2, 2, "00000000090807FF010203FF090807FF")));
    check("empty_map", with(p(2, 1, 0x20, 0), &[0, 5]), Some((2, 1, "FFFFFF00FFFFFF00")));
    check("id_skipped", with(pal(1, 1, 0x20, 1, 2, 1, 24, 8), &[65, 66, 4, 5, 6, 0]), Some((1, 1, "040506FF")));
    let big: Vec<u8> = (0..=255u8).flat_map(|i| [i, 255 - i, 1]).collect();
    check("index_255", with(p(2, 1, 0x20, 256), &[&big[..], &[255, 0]].concat()), Some((2, 1, "FF0001FF00FF01FF")));
    check("index_eq_len", with(p(2, 1, 0x20, 2), &[1, 1, 1, 2, 2, 2, 1, 2]), Some((2, 1, "020202FFFFFFFF00")));
    check("near_black", with(p(3, 1, 0x20, 3), &[0, 0, 1, 0, 1, 0, 1, 0, 0, 0, 1, 2]), Some((3, 1, "000001FF000100FF010000FF")));
    check("trailing", with(p(1, 1, 0x20, 1), &[1, 2, 3, 0, 99, 99]), Some((1, 1, "010203FF")));
    let mut first = p(1, 1, 0x20, 1);
    first[3] = 7; // colour-map first-entry index is ignored
    check("first_entry", with(first, &[1, 2, 3, 0]), Some((1, 1, "010203FF")));
    check("desc_extra_bits", with(p(1, 2, 0x38, 1), &[1, 2, 3, 0, 0]), Some((1, 2, "010203FF010203FF")));
}

#[test]
fn paletted_rejections_match_csharp() {
    check("map_type0", with(pal(1, 1, 0x20, 1, 0, 0, 24, 8), &[1, 2, 3, 0]), None);
    check("entry16", with(pal(1, 1, 0x20, 1, 0, 1, 16, 8), &[1, 2, 0]), None);
    check("entry32", with(pal(1, 1, 0x20, 1, 0, 1, 32, 8), &[1, 2, 3, 4, 0]), None);
    check("depth16", with(pal(1, 1, 0x20, 1, 0, 1, 24, 16), &[1, 2, 3, 0, 0]), None);
    check("short_pixels", with(p(2, 1, 0x20, 1), &[1, 2, 3, 0]), None);
    check("short_palette", with(p(1, 1, 0x20, 2), &[1, 2, 3, 4]), None);
}

#[test]
fn truecolor_edges_match_csharp() {
    check("desc_extra_top_down_32", with(header(2, 1, 2, 32, 0x28, 0), &[1, 2, 3, 4, 5, 6, 7, 8]), Some((1, 2, "0102030405060708")));
    check("right_to_left_ignored", with(header(2, 2, 1, 24, 0x30, 0), &[1, 2, 3, 4, 5, 6]), Some((2, 1, "010203FF040506FF")));
    check("id_255", with(header(2, 1, 1, 24, 0x20, 255), &[&[0u8; 255][..], &[1, 2, 3]].concat()), Some((1, 1, "010203FF")));
    let mut cm = header(2, 1, 1, 24, 0x20, 0);
    cm[1] = 1;
    cm[5] = 9; // colour-map fields never shift type-2 pixel data
    check("colormap_ignored", with(cm, &[1, 2, 3]), Some((1, 1, "010203FF")));
    check("trailing", with(header(2, 1, 1, 24, 0x20, 0), &[1, 2, 3, 9, 9, 9]), Some((1, 1, "010203FF")));
    let px: Vec<u8> = (1..=24).collect();
    check("3x2_bottom_up_32", with(header(2, 3, 2, 32, 0, 0), &px), Some((3, 2, "0D0E0F1011121314151617180102030405060708090A0B0C")));
}

#[test]
fn truecolor_rejections_and_limits_match_csharp() {
    check("trunc_24", with(header(2, 1, 2, 24, 0, 0), &[1, 2, 3, 4, 5]), None);
    check("trunc_32", with(header(2, 1, 2, 32, 0, 0), &[1, 2, 3, 4, 5, 6, 7]), None);
    check("depth16", with(header(2, 1, 1, 16, 0, 0), &[1, 2]), None);
    check("depth8", with(header(2, 1, 1, 8, 0, 0), &[1]), None);
    check("rle10", with(header(10, 1, 1, 24, 0, 0), &[1, 2, 3]), None);
    check("rle9", with(header(9, 1, 1, 8, 0, 0), &[1]), None);
    check("type0", with(header(0, 1, 1, 24, 0, 0), &[1, 2, 3]), None);
    check("zero_height", header(2, 1, 0, 24, 0, 0), None);
    check("zero_both", header(2, 0, 0, 24, 0, 0), None);
    check("id_len_overruns", with(header(2, 1, 1, 24, 0x20, 10), &[1, 2, 3]), None);
    check("header_only", header(2, 1, 1, 24, 0, 0), None);
    check("max_dims", with(header(2, 65535, 65535, 24, 0, 0), &[1, 2, 3]), None);
    check("exactly_2gib_output", header(2, 32768, 16384, 32, 0, 0), None);
    check("wide_truncated", with(header(2, 65535, 1, 24, 0, 0), &[1, 2, 3]), None);
}
