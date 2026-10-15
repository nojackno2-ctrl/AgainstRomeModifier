// Fixtures ported verbatim from native/argm-trace/tests/lde_test.cpp.
use winmm::lde::decode_length;
#[test]
fn cpp_case_0() {
    assert_eq!(decode_length(&[0x53]), Some(1), "push ebx");
}
#[test]
fn cpp_case_1() {
    assert_eq!(decode_length(&[0x55]), Some(1), "push ebp");
}
#[test]
fn cpp_case_2() {
    assert_eq!(
        decode_length(&[0x8B, 0x5C, 0x24, 0x08]),
        Some(4),
        "mov ebx,[esp+8]"
    );
}
#[test]
fn cpp_case_3() {
    assert_eq!(decode_length(&[0x8B, 0xFF]), Some(2), "mov edi,edi");
}
#[test]
fn cpp_case_4() {
    assert_eq!(decode_length(&[0x8B, 0xEC]), Some(2), "mov ebp,esp");
}
#[test]
fn cpp_case_5() {
    assert_eq!(
        decode_length(&[0x8B, 0x44, 0x24, 0x04]),
        Some(4),
        "mov eax,[esp+4]"
    );
}
#[test]
fn cpp_case_6() {
    assert_eq!(decode_length(&[0x83, 0xEC, 0x10]), Some(3), "sub esp,0x10");
}
#[test]
fn cpp_case_7() {
    assert_eq!(
        decode_length(&[0x81, 0xEC, 0x00, 0x01, 0x00, 0x00]),
        Some(6),
        "sub esp,0x100"
    );
}
#[test]
fn cpp_case_8() {
    assert_eq!(
        decode_length(&[0xB8, 0x44, 0x33, 0x22, 0x11]),
        Some(5),
        "mov eax,imm32"
    );
}
#[test]
fn cpp_case_9() {
    assert_eq!(
        decode_length(&[0x68, 0x44, 0x33, 0x22, 0x11]),
        Some(5),
        "push imm32"
    );
}
#[test]
fn cpp_case_10() {
    assert_eq!(decode_length(&[0x6A, 0x04]), Some(2), "push imm8");
}
#[test]
fn cpp_case_11() {
    assert_eq!(decode_length(&[0x33, 0xC0]), Some(2), "xor eax,eax");
}
#[test]
fn cpp_case_12() {
    assert_eq!(
        decode_length(&[0x89, 0x5C, 0x24, 0x04]),
        Some(4),
        "mov [esp+4],ebx"
    );
}
#[test]
fn cpp_case_13() {
    assert_eq!(
        decode_length(&[0x8D, 0x44, 0x24, 0x08]),
        Some(4),
        "lea eax,[esp+8]"
    );
}
#[test]
fn cpp_case_14() {
    assert_eq!(
        decode_length(&[0x8B, 0x0D, 0x78, 0x74, 0x73, 0x00]),
        Some(6),
        "mov ecx,ds:[imm32]"
    );
}
#[test]
fn cpp_case_15() {
    assert_eq!(
        decode_length(&[0x89, 0x1D, 0x78, 0x74, 0x73, 0x00]),
        Some(6),
        "mov ds:[imm32],ebx"
    );
}
#[test]
fn cpp_case_16() {
    assert_eq!(decode_length(&[0xC3]), Some(1), "ret");
}
#[test]
fn cpp_case_17() {
    assert_eq!(decode_length(&[0xC2, 0x08, 0x00]), Some(3), "ret imm16");
}
#[test]
fn cpp_case_18() {
    assert_eq!(decode_length(&[0xCA, 0x08, 0x00]), Some(3), "retf imm16");
}
#[test]
fn cpp_case_19() {
    assert_eq!(
        decode_length(&[0x3B, 0x43, 0x28]),
        Some(3),
        "cmp eax,[ebx+0x28]"
    );
}
#[test]
fn cpp_case_20() {
    assert_eq!(decode_length(&[0x83, 0xC4, 0x04]), Some(3), "add esp,4");
}
#[test]
fn cpp_case_21() {
    assert_eq!(decode_length(&[0x90]), Some(1), "nop");
}
#[test]
fn cpp_case_22() {
    assert_eq!(
        decode_length(&[0x0F, 0xB6, 0x02]),
        Some(3),
        "movzx eax,byte[edx]"
    );
}
#[test]
fn cpp_case_23() {
    assert_eq!(
        decode_length(&[0x0F, 0xB6, 0x42, 0x01]),
        Some(4),
        "movzx eax,byte[edx+1]"
    );
}
#[test]
fn cpp_case_24() {
    assert_eq!(decode_length(&[0xA8, 0x01]), Some(2), "test al,imm8");
}
#[test]
fn cpp_case_25() {
    assert_eq!(
        decode_length(&[0xA9, 0x01, 0x00, 0x00, 0x00]),
        Some(5),
        "test eax,imm32"
    );
}
#[test]
fn cpp_case_26() {
    assert_eq!(
        decode_length(&[0xE8, 0x00, 0x10, 0x00, 0x00]),
        Some(5),
        "call rel32"
    );
}
#[test]
fn cpp_case_27() {
    assert_eq!(
        decode_length(&[0xE9, 0x00, 0x10, 0x00, 0x00]),
        Some(5),
        "jmp rel32"
    );
}
#[test]
fn cpp_case_28() {
    assert_eq!(decode_length(&[0xEB, 0x10]), Some(2), "jmp rel8");
}
#[test]
fn cpp_case_29() {
    assert_eq!(decode_length(&[0x74, 0x10]), Some(2), "jz rel8");
}
#[test]
fn cpp_case_30() {
    assert_eq!(
        decode_length(&[0x0F, 0x85, 0x00, 0x10, 0x00, 0x00]),
        Some(6),
        "jnz rel32"
    );
}
#[test]
fn cpp_case_31() {
    assert_eq!(
        decode_length(&[0x8B, 0x04, 0x85, 0x00, 0x00, 0x00, 0x00]),
        Some(7),
        "mov eax,[eax*4+imm32]"
    );
}
#[test]
fn cpp_case_32() {
    assert_eq!(
        decode_length(&[0xC7, 0x45, 0xFC, 0x00, 0x00, 0x00, 0x00]),
        Some(7),
        "mov r/m,imm32(C7)"
    );
}
#[test]
fn cpp_case_33() {
    assert_eq!(
        decode_length(&[0xF7, 0xC0, 0x01, 0x00, 0x00, 0x00]),
        Some(6),
        "test r/m,imm32(F7/0)"
    );
}
#[test]
fn cpp_case_34() {
    assert_eq!(decode_length(&[0xF7, 0xD8]), Some(2), "neg eax (F7/3)");
}
#[test]
fn cpp_case_35() {
    assert_eq!(
        decode_length(&[0xFF, 0x00]),
        Some(2),
        "inc dword[eax](FF/0)"
    );
}
#[test]
fn cpp_case_36() {
    assert_eq!(
        decode_length(&[0xFF, 0x15, 0x00, 0x00, 0x00, 0x00]),
        Some(6),
        "call [imm32](FF/2)"
    );
}
#[test]
fn cpp_case_37() {
    assert_eq!(
        decode_length(&[0x66, 0x8B, 0xFF]),
        Some(3),
        "mov edi,edi(prefix66)"
    );
}
#[test]
fn cpp_case_38() {
    assert_eq!(
        decode_length(&[0x66, 0x68, 0x00, 0x10]),
        Some(4),
        "push imm16(66 68)"
    );
}
#[test]
fn cpp_case_39() {
    assert_eq!(decode_length(&[0xC8, 0x10, 0x00, 0x00]), Some(4), "enter");
}
#[test]
fn cpp_case_40() {
    assert_eq!(decode_length(&[0x0F, 0x95, 0xC0]), Some(3), "setne al");
}
#[test]
fn cpp_case_41() {
    assert_eq!(decode_length(&[0x0F, 0x44, 0xC1]), Some(3), "cmovz eax,ecx");
}
#[test]
fn cpp_case_42() {
    assert_eq!(
        decode_length(&[0x6B, 0xC1, 0x04]),
        Some(3),
        "imul eax,ecx,imm8"
    );
}
#[test]
fn cpp_case_43() {
    assert_eq!(
        decode_length(&[0x83, 0xE4, 0xF0]),
        Some(3),
        "and esp,-16(83)"
    );
}
#[test]
fn cpp_case_44() {
    assert_eq!(decode_length(&[0x89, 0xE5]), Some(2), "mov ebp,esp(89E5)");
}
#[test]
fn cpp_case_45() {
    assert_eq!(decode_length(&[0x5B]), Some(1), "pop ebx");
}
#[test]
fn cpp_case_46() {
    assert_eq!(decode_length(&[0x31, 0xFF]), Some(2), "xor edi,edi(31FF)");
}
