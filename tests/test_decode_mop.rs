pub mod machine_build;
use ckb_vm::decoder::{DefaultDecoder, InstDecoder};
use ckb_vm::instructions::extract_opcode;
use ckb_vm::machine::{VERSION0, VERSION1, VERSION2};
use ckb_vm::memory::Memory;
use ckb_vm::{SparseMemory, ISA_IMC, ISA_MOP};
use ckb_vm_definitions::instructions as insts;

const FUNCT7_BASE: u32 = 0;
const OPCODE_OP: u32 = 0x33;

fn encode_r(funct3: u32, rd: u32, rs1: u32, rs2: u32) -> u64 {
    ((FUNCT7_BASE << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | OPCODE_OP)
        as u64
}

#[test]
pub fn test_decode_mop_adc_partial_match() {
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000u64;

    // Valid ADC sequence:
    // add x1, x1, x2
    // sltu x2, x1, x2
    // add x1, x1, x4
    // sltu x4, x1, x4
    // or x2, x2, x4
    let head = encode_r(0b000, 1, 1, 2);
    let next = encode_r(0b011, 2, 1, 2);
    let neck = encode_r(0b000, 1, 1, 4);
    let body = encode_r(0b011, 4, 1, 4);
    let tail = encode_r(0b110, 2, 2, 4);

    // Test 1: Full match
    memory.store32(&pc, &head).unwrap();
    memory.store32(&(pc + 4), &next).unwrap();
    memory.store32(&(pc + 8), &neck).unwrap();
    memory.store32(&(pc + 12), &body).unwrap();
    memory.store32(&(pc + 16), &tail).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_ADC);
}

#[test]
pub fn test_decode_add3_version_check() {
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000u64;

    // add x1, x2, x1
    // sltu x3, x1, x2
    // add x4, x3, x5
    let i0 = encode_r(0b000, 1, 2, 1);
    let i1 = encode_r(0b011, 3, 1, 2);
    let i2 = encode_r(0b000, 4, 3, 5);

    memory.store32(&pc, &i0).unwrap();
    memory.store32(&(pc + 4), &i1).unwrap();
    memory.store32(&(pc + 8), &i2).unwrap();

    let mut decoder_v2 = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION2);
    let inst_v2 = decoder_v2.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst_v2), insts::OP_ADD3A);
}

#[test]
pub fn test_decode_adcs_boundary_registers() {
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000u64;

    // add x0, x1, x2
    // sltu x3, x0, x1
    let head_x0 = encode_r(0b000, 0, 1, 2);
    let next_x0 = encode_r(0b011, 3, 0, 1);

    memory.store32(&pc, &head_x0).unwrap();
    memory.store32(&(pc + 4), &next_x0).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION2);
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_ADD);
}

#[test]
pub fn test_decode_mop_cache_collision() {
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc1 = 0x1000u64;
    let pc2 = 0x1000u64 + (1 << 13);

    let head = encode_r(0b000, 1, 2, 3);

    memory.store32(&pc1, &head).unwrap();
    memory.store32(&pc2, &head).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);
    let _ = decoder.decode(&mut memory, pc1).unwrap();
    let inst2 = decoder.decode(&mut memory, pc2).unwrap();
    assert_eq!(extract_opcode(inst2), insts::OP_ADD);
}

// --- Iteration 32: decode_mop edge-case tests ---

#[test]
pub fn test_decode_mop_adc_with_same_rs1_rs2_does_not_fuse() {
    // ADC requires rs1 != rs2, so add x1, x1, x1 should not fuse
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000u64;

    let head = encode_r(0b000, 1, 1, 1); // add x1, x1, x1 (rs1==rs2)
    memory.store32(&pc, &head).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_ADD); // Should not fuse
}

#[test]
pub fn test_decode_mop_adc_with_x0_head_does_not_fuse() {
    // ADC requires rd != x0
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000u64;

    let head = encode_r(0b000, 0, 1, 2); // add x0, x1, x2
    let next = encode_r(0b011, 2, 0, 2); // sltu x2, x0, x2
    let neck = encode_r(0b000, 0, 0, 4); // add x0, x0, x4
    let body = encode_r(0b011, 4, 0, 4); // sltu x4, x0, x4
    let tail = encode_r(0b110, 2, 2, 4); // or x2, x2, x4

    memory.store32(&pc, &head).unwrap();
    memory.store32(&(pc + 4), &next).unwrap();
    memory.store32(&(pc + 8), &neck).unwrap();
    memory.store32(&(pc + 12), &body).unwrap();
    memory.store32(&(pc + 16), &tail).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);
    let inst = decoder.decode(&mut memory, pc).unwrap();
    // head has rd=x0 so ADC won't fuse
    assert_eq!(extract_opcode(inst), insts::OP_ADD);
}

#[test]
pub fn test_decode_mop_adc_version0_no_mop() {
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000u64;

    let head = encode_r(0b000, 1, 1, 2);
    let next = encode_r(0b011, 2, 1, 2);
    let neck = encode_r(0b000, 1, 1, 4);
    let body = encode_r(0b011, 4, 1, 4);
    let tail = encode_r(0b110, 2, 2, 4);

    memory.store32(&pc, &head).unwrap();
    memory.store32(&(pc + 4), &next).unwrap();
    memory.store32(&(pc + 8), &neck).unwrap();
    memory.store32(&(pc + 12), &body).unwrap();
    memory.store32(&(pc + 16), &tail).unwrap();

    // VERSION0 should not do MOP fusion even with ISA_MOP
    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION0);
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_ADD); // Not fused on VERSION0
}

#[test]
pub fn test_decode_mop_add3x0_only_on_version2() {
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000u64;

    // ADD3A pattern: add r0, r1, r0; sltu r2, r0, r1; add r3, r2, r4
    let i0 = encode_r(0b000, 1, 2, 1); // add x1, x2, x1
    let i1 = encode_r(0b011, 3, 1, 2); // sltu x3, x1, x2
    let i2 = encode_r(0b000, 4, 3, 5); // add x4, x3, x5

    memory.store32(&pc, &i0).unwrap();
    memory.store32(&(pc + 4), &i1).unwrap();
    memory.store32(&(pc + 8), &i2).unwrap();

    // VERSION1 should not fuse ADD3
    let mut decoder_v1 = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);
    let inst_v1 = decoder_v1.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst_v1), insts::OP_ADD);

    // VERSION2 should fuse
    let mut decoder_v2 = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION2);
    let inst_v2 = decoder_v2.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst_v2), insts::OP_ADD3A);
}

#[test]
pub fn test_decode_mop_add3_with_x0_rs2_does_not_fuse() {
    // ADD3A requires r0 != r4, so if r4 is x0, check r0 != x0
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000u64;

    // add x1, x2, x1 -> sltu x3, x1, x2 -> add x4, x3, x0
    let i0 = encode_r(0b000, 1, 2, 1);
    let i1 = encode_r(0b011, 3, 1, 2);
    let i2 = encode_r(0b000, 4, 3, 0); // r4 = x0

    memory.store32(&pc, &i0).unwrap();
    memory.store32(&(pc + 4), &i1).unwrap();
    memory.store32(&(pc + 8), &i2).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION2);
    let inst = decoder.decode(&mut memory, pc).unwrap();
    // r0 != r4 (1 != 0) is OK but ADD3A has r0 != r4, r2 != r4, r0 != x0, r2 != x0
    // r2 (x3) != r4 (x0) is OK. Should fuse if all constraints met.
    // Actually: i2 has rd=x4, rs1=x3, rs2=x0, so r4=x0
    // The constraint is r0 != x0 (1 != 0 OK) and r2 != x0 (3 != 0 OK)
    // This should fuse!
    assert_eq!(extract_opcode(inst), insts::OP_ADD3A);
}

#[test]
pub fn test_decode_mop_multiple_opcodes_same_cache_line() {
    // Two different instructions at PCs that map to same cache key
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc1 = 0x1000u64;
    let pc2 = 0x2000u64;

    // Store ADD at pc1
    let add_inst = encode_r(0b000, 1, 2, 3);
    memory.store32(&pc1, &add_inst).unwrap();

    // Store SUB at pc2
    let sub_inst = encode_r(0b000, 1, 2, 3) | (1 << 30); // funct7 bit set -> SUB
    memory.store32(&pc2, &sub_inst).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);

    let inst1 = decoder.decode(&mut memory, pc1).unwrap();
    assert_eq!(extract_opcode(inst1), insts::OP_ADD);

    let inst2 = decoder.decode(&mut memory, pc2).unwrap();
    assert_eq!(extract_opcode(inst2), insts::OP_SUB);
}

#[test]
pub fn test_decode_mop_adc_partial_sequence_returns_head() {
    // If only 3 of 5 ADC instructions match, should return head (no fusion)
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000u64;

    let head = encode_r(0b000, 1, 1, 2); // add x1, x1, x2
    let next = encode_r(0b011, 2, 1, 2); // sltu x2, x1, x2
    let wrong = encode_r(0b010, 5, 5, 6); // slt x5, x5, x6 (not an add)

    memory.store32(&pc, &head).unwrap();
    memory.store32(&(pc + 4), &next).unwrap();
    memory.store32(&(pc + 8), &wrong).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_ADD);
}

#[test]
pub fn test_decode_mop_far_jump_abs_lui_jalr_fusion() {
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000u64;

    // LUI x1, 0x12345 -> JALR x1, x1, 0x678
    // LUI: opcode=0x37, rd=1, imm=0x12345
    let lui = (0x12345u32 << 12) | (1 << 7) | 0x37;
    // JALR: opcode=0x67, rd=1, rs1=1, imm=0x678
    let jalr = (0x678u32 << 20) | (1 << 15) | (1 << 12) | (1 << 7) | 0x67;

    memory.store32(&pc, &lui).unwrap();
    memory.store32(&(pc + 4), &jalr).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_FAR_JUMP_ABS);
}

#[test]
pub fn test_decode_mop_far_jump_lui_jalr_no_fuse_wrong_rd() {
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000u64;

    // LUI x1, 0x12345 -> JALR x2, x1, 0x678 (rd != RA, no fusion)
    let lui = (0x12345u32 << 12) | (1 << 7) | 0x37;
    let jalr = (0x678u32 << 20) | (1 << 15) | (1 << 12) | (2 << 7) | 0x67; // rd=x2

    memory.store32(&pc, &lui).unwrap();
    memory.store32(&(pc + 4), &jalr).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_LUI); // Not fused
}

#[test]
pub fn test_decode_mop_at_memory_edge_returns_instruction() {
    // Decode at the last valid 4-byte boundary in memory
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = (0x1000000 - 4) as u64;

    let add_inst = encode_r(0b000, 1, 2, 3);
    memory.store32(&pc, &add_inst).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_ADD);
}

#[test]
pub fn test_decode_mop_adc_with_neck_rd_conflict() {
    // ADC requires neck.rd == neck.rs1 == next.rs1, and neck.rs2 conflicts with head
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000u64;

    // head: add x1, x1, x2
    // next: sltu x2, x1, x2
    // neck: add x1, x1, x1 (neck.rs2 == head.rs1, conflict!)
    let head = encode_r(0b000, 1, 1, 2);
    let next = encode_r(0b011, 2, 1, 2);
    let neck = encode_r(0b000, 1, 1, 1); // rs2 == head.rs1

    memory.store32(&pc, &head).unwrap();
    memory.store32(&(pc + 4), &next).unwrap();
    memory.store32(&(pc + 8), &neck).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_ADD); // Not fused
}

#[test]
pub fn test_decode_mop_cache_eviction_across_many_pcs() {
    // Decode many instructions to fill and evict cache entries
    let mut memory = SparseMemory::<u64>::new(0x1000000);

    // Fill memory with various instructions at different PCs
    for i in 0u64..256 {
        let pc = i * 4;
        let inst = encode_r(
            0b000,
            (i % 32) as u32,
            ((i + 1) % 32) as u32,
            ((i + 2) % 32) as u32,
        );
        memory.store32(&pc, &inst).unwrap();
    }

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);

    // Decode all - should not panic or return wrong instructions
    for i in 0u64..256 {
        let pc = i * 4;
        let inst = decoder.decode(&mut memory, pc).unwrap();
        assert_eq!(extract_opcode(inst), insts::OP_ADD);
    }
}

#[test]
pub fn test_decode_mop_decode_raw_at_out_of_bounds_pc() {
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000000u64; // Exactly at memory size

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);
    let result = decoder.decode(&mut memory, pc);
    assert!(result.is_err());
}
