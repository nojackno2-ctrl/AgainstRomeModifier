//! Tiny JSON helpers (no dependencies). Non-finite floats become `null`; raw bits are always emitted next to them.

use crate::pool::LightSnapshot;

pub fn string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

pub fn float(v: f32) -> String {
    if v.is_finite() {
        format!("{v}")
    } else {
        "null".to_string()
    }
}

fn float3(v: [f32; 3]) -> String {
    format!("[{},{},{}]", float(v[0]), float(v[1]), float(v[2]))
}
fn bits3(v: [u32; 3]) -> String {
    format!("[\"0x{:08x}\",\"0x{:08x}\",\"0x{:08x}\"]", v[0], v[1], v[2])
}

/// Saved-state evidence only: values exactly as stored, no derived/runtime fields.
pub fn snapshot(s: &LightSnapshot) -> String {
    let raw: String = s.raw_fields().iter().map(|b| format!("{b:02x}")).collect();
    format!(
        concat!(
            "{{\"slot\":{},\"active_raw\":{},\"position\":{},\"position_bits\":{},",
            "\"color\":{},\"color_bits\":{},\"effective_radius\":{},\"effective_radius_bits\":\"0x{:08x}\",",
            "\"baseline_radius\":{},\"baseline_radius_bits\":\"0x{:08x}\",\"radii_equal_bits\":{},",
            "\"type_raw\":{},\"type_parameter\":{},\"type_parameter_bits\":\"0x{:08x}\",",
            "\"age_raw\":{},\"age_signed\":{},\"special_fx_raw\":{},",
            "\"owner\":{},\"parent\":{},\"definition\":{},\"raw_fields_hex\":\"{}\"}}"
        ),
        s.slot(),
        s.active_raw(),
        float3(s.position()),
        bits3(s.position_bits()),
        float3(s.color()),
        bits3(s.color_bits()),
        float(s.effective_radius()),
        s.effective_radius_bits(),
        float(s.baseline_radius()),
        s.baseline_radius_bits(),
        s.effective_radius_bits() == s.baseline_radius_bits(),
        s.type_raw(),
        float(s.type_parameter()),
        s.type_parameter_bits(),
        s.age_raw(),
        s.age_signed(),
        s.special_fx_raw(),
        s.owner(),
        s.parent(),
        s.definition(),
        raw
    )
}
