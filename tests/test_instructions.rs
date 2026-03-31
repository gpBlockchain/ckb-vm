use ckb_vm::instructions::{
    blank_instruction, extract_opcode, instruction_length, is_basic_block_end_instruction,
    is_slowpath_instruction, set_instruction_length_2, set_instruction_length_4,
    set_instruction_length_n, Instruction, Itype, R4type, R5type, Rtype, Stype, Utype,
};

// =========================================================================
// Rtype boundary tests
// =========================================================================

#[test]
fn test_rtype_roundtrip_max_values() {
    // Max register index is 31 (5 bits)
    let inst = Rtype::new(0xFFFF, 31, 31, 31);
    assert_eq!(inst.rd(), 31);
    assert_eq!(inst.rs1(), 31);
    assert_eq!(inst.rs2(), 31);
}

#[test]
fn test_rtype_roundtrip_zero_values() {
    let inst = Rtype::new(0, 0, 0, 0);
    assert_eq!(inst.rd(), 0);
    assert_eq!(inst.rs1(), 0);
    assert_eq!(inst.rs2(), 0);
}

#[test]
fn test_rtype_roundtrip_mid_values() {
    let inst = Rtype::new(0x1234, 15, 7, 23);
    assert_eq!(inst.rd(), 15);
    assert_eq!(inst.rs1(), 7);
    assert_eq!(inst.rs2(), 23);
}

#[test]
fn test_rtype_opcode_roundtrip() {
    for op in [0x0000, 0x0033, 0x3300, 0x7FFF, 0xFFFF] {
        let inst = Rtype::new(op, 1, 2, 3);
        assert_eq!(inst.op(), op, "Opcode {:#x} should roundtrip", op);
    }
}

#[test]
fn test_rtype_register_independence() {
    // Verify rd, rs1, rs2 don't interfere with each other
    let inst = Rtype::new(0x33, 0, 31, 0);
    assert_eq!(inst.rd(), 0);
    assert_eq!(inst.rs1(), 31);
    assert_eq!(inst.rs2(), 0);

    let inst = Rtype::new(0x33, 31, 0, 31);
    assert_eq!(inst.rd(), 31);
    assert_eq!(inst.rs1(), 0);
    assert_eq!(inst.rs2(), 31);
}

// =========================================================================
// Itype boundary tests
// =========================================================================

#[test]
fn test_itype_roundtrip_max_values() {
    let inst = Itype::new_u(0xFFFF, 31, 31, 0xFFF);
    assert_eq!(inst.rd(), 31);
    assert_eq!(inst.rs1(), 31);
    assert_eq!(inst.immediate_u(), 0xFFF);
}

#[test]
fn test_itype_roundtrip_zero_values() {
    let inst = Itype::new_u(0, 0, 0, 0);
    assert_eq!(inst.rd(), 0);
    assert_eq!(inst.rs1(), 0);
    assert_eq!(inst.immediate_u(), 0);
}

#[test]
fn test_itype_signed_immediate_negative() {
    // -1 as signed immediate
    let inst = Itype::new_s(0x13, 1, 2, -1);
    assert_eq!(inst.rd(), 1);
    assert_eq!(inst.rs1(), 2);
    assert_eq!(inst.immediate_s(), -1);
}

#[test]
fn test_itype_signed_immediate_min() {
    // Min 12-bit signed: -2048
    let inst = Itype::new_s(0x13, 1, 2, -2048);
    assert_eq!(inst.immediate_s(), -2048);
}

#[test]
fn test_itype_signed_immediate_max() {
    // Max 12-bit signed: 2047
    let inst = Itype::new_s(0x13, 1, 2, 2047);
    assert_eq!(inst.immediate_s(), 2047);
}

#[test]
fn test_itype_opcode_roundtrip() {
    for op in [0x0000, 0x0013, 0x1300, 0x7FFF, 0xFFFF] {
        let inst = Itype::new_u(op, 1, 2, 100);
        assert_eq!(inst.op(), op, "Opcode {:#x} should roundtrip", op);
    }
}

// =========================================================================
// Stype boundary tests
// =========================================================================

#[test]
fn test_stype_roundtrip_max_values() {
    let inst = Stype::new_u(0xFFFF, 0xFFF, 31, 31);
    assert_eq!(inst.rs1(), 31);
    assert_eq!(inst.rs2(), 31);
    assert_eq!(inst.immediate_u(), 0xFFF);
}

#[test]
fn test_stype_roundtrip_zero_values() {
    let inst = Stype::new_u(0, 0, 0, 0);
    assert_eq!(inst.rs1(), 0);
    assert_eq!(inst.rs2(), 0);
    assert_eq!(inst.immediate_u(), 0);
}

#[test]
fn test_stype_signed_immediate_negative() {
    let inst = Stype::new_s(0x23, -1, 1, 2);
    assert_eq!(inst.immediate_s(), -1);
}

#[test]
fn test_stype_signed_immediate_boundary() {
    // Min 12-bit signed: -2048
    let inst = Stype::new_s(0x23, -2048, 1, 2);
    assert_eq!(inst.immediate_s(), -2048);

    // Max 12-bit signed: 2047
    let inst = Stype::new_s(0x23, 2047, 1, 2);
    assert_eq!(inst.immediate_s(), 2047);
}

#[test]
fn test_stype_opcode_roundtrip() {
    for op in [0x0000, 0x0023, 0x2300, 0x7FFF, 0xFFFF] {
        let inst = Stype::new_u(op, 100, 1, 2);
        assert_eq!(inst.op(), op, "Opcode {:#x} should roundtrip", op);
    }
}

// =========================================================================
// Utype boundary tests
// =========================================================================

#[test]
fn test_utype_roundtrip_max_values() {
    let inst = Utype::new(0xFFFF, 31, 0xFFFFF);
    assert_eq!(inst.rd(), 31);
    assert_eq!(inst.immediate_u(), 0xFFFFF);
}

#[test]
fn test_utype_roundtrip_zero_values() {
    let inst = Utype::new(0, 0, 0);
    assert_eq!(inst.rd(), 0);
    assert_eq!(inst.immediate_u(), 0);
}

#[test]
fn test_utype_signed_immediate() {
    let inst = Utype::new_s(0x37, 1, -1);
    assert_eq!(inst.immediate_s(), -1);
}

#[test]
fn test_utype_signed_immediate_min() {
    // Min 20-bit signed: -524288
    let inst = Utype::new_s(0x37, 1, -524288);
    assert_eq!(inst.immediate_s(), -524288);
}

#[test]
fn test_utype_signed_immediate_max() {
    // Max 20-bit signed: 524287
    let inst = Utype::new_s(0x37, 1, 524287);
    assert_eq!(inst.immediate_s(), 524287);
}

#[test]
fn test_utype_opcode_roundtrip() {
    for op in [0x0000, 0x0037, 0x3700, 0x6F00, 0xFFFF] {
        let inst = Utype::new(op, 1, 0x12345);
        assert_eq!(inst.op(), op, "Opcode {:#x} should roundtrip", op);
    }
}

// =========================================================================
// R4type boundary tests
// =========================================================================

#[test]
fn test_r4type_roundtrip_max_values() {
    let inst = R4type::new(0xFFFF, 31, 31, 31, 31);
    assert_eq!(inst.rd(), 31);
    assert_eq!(inst.rs1(), 31);
    assert_eq!(inst.rs2(), 31);
    assert_eq!(inst.rs3(), 31);
}

#[test]
fn test_r4type_roundtrip_zero_values() {
    let inst = R4type::new(0, 0, 0, 0, 0);
    assert_eq!(inst.rd(), 0);
    assert_eq!(inst.rs1(), 0);
    assert_eq!(inst.rs2(), 0);
    assert_eq!(inst.rs3(), 0);
}

#[test]
fn test_r4type_register_independence() {
    let inst = R4type::new(0x33, 1, 2, 3, 4);
    assert_eq!(inst.rd(), 1);
    assert_eq!(inst.rs1(), 2);
    assert_eq!(inst.rs2(), 3);
    assert_eq!(inst.rs3(), 4);
}

// =========================================================================
// R5type boundary tests
// =========================================================================

#[test]
fn test_r5type_roundtrip_max_values() {
    let inst = R5type::new(0xFFFF, 31, 31, 31, 31, 31);
    assert_eq!(inst.rd(), 31);
    assert_eq!(inst.rs1(), 31);
    assert_eq!(inst.rs2(), 31);
    assert_eq!(inst.rs3(), 31);
    assert_eq!(inst.rs4(), 31);
}

#[test]
fn test_r5type_roundtrip_zero_values() {
    let inst = R5type::new(0, 0, 0, 0, 0, 0);
    assert_eq!(inst.rd(), 0);
    assert_eq!(inst.rs1(), 0);
    assert_eq!(inst.rs2(), 0);
    assert_eq!(inst.rs3(), 0);
    assert_eq!(inst.rs4(), 0);
}

#[test]
fn test_r5type_register_independence() {
    let inst = R5type::new(0x33, 1, 2, 3, 4, 5);
    assert_eq!(inst.rd(), 1);
    assert_eq!(inst.rs1(), 2);
    assert_eq!(inst.rs2(), 3);
    assert_eq!(inst.rs3(), 4);
    assert_eq!(inst.rs4(), 5);
}

// =========================================================================
// extract_opcode / blank_instruction boundary tests
// =========================================================================

#[test]
fn test_extract_opcode_zero() {
    assert_eq!(extract_opcode(0), 0);
}

#[test]
fn test_extract_opcode_max() {
    assert_eq!(extract_opcode(u64::MAX), 0xFFFF);
}

#[test]
fn test_blank_instruction_roundtrip() {
    for op in [0x0000, 0x0033, 0x3300, 0x7FFF, 0xFFFF] {
        let inst = blank_instruction(op);
        assert_eq!(extract_opcode(inst), op, "Opcode {:#x} roundtrip", op);
    }
}

#[test]
fn test_extract_opcode_preserves_low_byte() {
    // The opcode encoding uses bits 0-7 and 16-23
    let inst = 0x00_00_00_33u64; // op = 0x33
    assert_eq!(extract_opcode(inst), 0x33);
}

#[test]
fn test_extract_opcode_preserves_high_byte() {
    // Use blank_instruction to create an instruction with high opcode byte
    let inst = blank_instruction(0x3300);
    assert_eq!(extract_opcode(inst), 0x3300);
}

// =========================================================================
// instruction_length tests (length is encoded, not derived from raw bits)
// =========================================================================

#[test]
fn test_instruction_length_16bit() {
    // Must use set_instruction_length_2 to encode the length
    let inst = set_instruction_length_2(0x0001);
    assert_eq!(instruction_length(inst), 2);
}

#[test]
fn test_instruction_length_32bit() {
    let inst = set_instruction_length_4(0x0003);
    assert_eq!(instruction_length(inst), 4);
}

#[test]
fn test_instruction_length_set() {
    let inst = 0x0003u64;
    let inst = set_instruction_length_2(inst);
    assert_eq!(instruction_length(inst), 2);

    let inst = 0x0003u64;
    let inst = set_instruction_length_4(inst);
    assert_eq!(instruction_length(inst), 4);
}

#[test]
fn test_instruction_length_n() {
    // n must be even (multiple of 2)
    for len in [2u8, 4, 6, 8, 10, 12, 14, 16] {
        let inst = set_instruction_length_n(0x0003, len);
        assert_eq!(
            instruction_length(inst),
            len,
            "Length {} should roundtrip",
            len
        );
    }
}

#[test]
fn test_instruction_length_n_max_even() {
    // Max valid even value <= 30
    let inst = set_instruction_length_n(0x0003, 30);
    assert_eq!(instruction_length(inst), 30);
}

#[test]
fn test_instruction_length_uninitialized() {
    // Raw instruction without set_instruction_length has length 0
    assert_eq!(instruction_length(0x0003), 0);
    assert_eq!(instruction_length(0x0001), 0);
}

// =========================================================================
// is_basic_block_end_instruction / is_slowpath_instruction
// =========================================================================

#[test]
fn test_is_basic_block_end_instruction() {
    // This function checks opcode ranges, not instruction length
    // Just verify it doesn't panic for various inputs
    for i in [0u64, 1, 0x33, 0x63, 0x67, 0x6F, 0xFFFF, u64::MAX] {
        let _ = is_basic_block_end_instruction(i);
    }
}

// =========================================================================
// Cross-type register index boundary tests
// =========================================================================

#[test]
fn test_all_types_register_0() {
    // Register 0 (x0/zero) should work in all types
    let r = Rtype::new(0x33, 0, 0, 0);
    assert_eq!(r.rd(), 0);

    let i = Itype::new_u(0x13, 0, 0, 0);
    assert_eq!(i.rd(), 0);

    let s = Stype::new_u(0x23, 0, 0, 0);
    assert_eq!(s.rs1(), 0);

    let u = Utype::new(0x37, 0, 0);
    assert_eq!(u.rd(), 0);

    let r4 = R4type::new(0x43, 0, 0, 0, 0);
    assert_eq!(r4.rd(), 0);

    let r5 = R5type::new(0x53, 0, 0, 0, 0, 0);
    assert_eq!(r5.rd(), 0);
}

#[test]
fn test_all_types_register_31() {
    // Register 31 (max valid) should work in all types
    let r = Rtype::new(0x33, 31, 31, 31);
    assert_eq!(r.rd(), 31);

    let i = Itype::new_u(0x13, 31, 31, 0xFFF);
    assert_eq!(i.rd(), 31);

    let s = Stype::new_u(0x23, 0xFFF, 31, 31);
    assert_eq!(s.rs1(), 31);

    let u = Utype::new(0x37, 31, 0xFFFFF);
    assert_eq!(u.rd(), 31);
}
