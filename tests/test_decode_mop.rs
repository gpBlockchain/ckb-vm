pub mod machine_build;
use ckb_vm::decoder::{DefaultDecoder, InstDecoder};
use ckb_vm::instructions::{extract_opcode, instruction_length, Rtype};
use ckb_vm::machine::{VERSION1, VERSION2};
use ckb_vm::memory::Memory;
use ckb_vm::{SparseMemory, ISA_IMC, ISA_MOP};
use ckb_vm_definitions::instructions as insts;

#[test]
pub fn test_decode_mop_adc_partial_match() {
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc = 0x1000;

    // ADC sequence:
    // head: OP_ADD
    // next: OP_SLTU
    // neck: OP_ADD
    // body: OP_SLTU
    // tail: OP_OR

    // Valid ADC sequence:
    // add r1, r2, r1
    // sltu r2, r1, r2
    // add r3, r1, r4
    // sltu r4, r3, r1
    // or r2, r2, r4

    let head = Rtype::new(insts::OP_ADD, 1, 2, 1); // rd=1, rs1=2, rs2=1
    let next = Rtype::new(insts::OP_SLTU, 2, 1, 2); // rd=2, rs1=1, rs2=2
    let neck = Rtype::new(insts::OP_ADD, 3, 1, 4); // rd=3, rs1=1, rs2=4
    let body = Rtype::new(insts::OP_SLTU, 4, 3, 1); // rd=4, rs1=3, rs2=1
    let tail = Rtype::new(insts::OP_OR, 2, 2, 4); // rd=2, rs1=2, rs2=4

    // Test 1: Full match
    memory.store32(pc, &head.0).unwrap();
    memory.store32(pc + 4, &next.0).unwrap();
    memory.store32(pc + 8, &neck.0).unwrap();
    memory.store32(pc + 12, &body.0).unwrap();
    memory.store32(pc + 16, &tail.0).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_ADC);
    assert_eq!(instruction_length(inst), 20);

    // Test 2: Partial match - fail at next (wrong opcode)
    let next_wrong = Rtype::new(insts::OP_SUB, 2, 1, 2);
    memory.store32(pc + 4, &next_wrong.0).unwrap();
    decoder.reset_instructions_cache().unwrap();
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_ADD);

    // Test 3: Partial match - fail at neck (wrong opcode)
    memory.store32(pc + 4, &next.0).unwrap();
    let neck_wrong = Rtype::new(insts::OP_SUB, 3, 1, 4);
    memory.store32(pc + 8, &neck_wrong.0).unwrap();
    decoder.reset_instructions_cache().unwrap();
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_ADD);

    // Test 4: Partial match - fail at body (wrong registers)
    memory.store32(pc + 8, &neck.0).unwrap();
    let body_wrong = Rtype::new(insts::OP_SLTU, 4, 3, 2); // rs2 should be 1
    memory.store32(pc + 12, &body_wrong.0).unwrap();
    decoder.reset_instructions_cache().unwrap();
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_ADD);
}

#[test]
pub fn test_decode_add3_version_check() {
    let mut memory = SparseMemory::<u64>::default();
    let pc = 0x1000;

    // ADD3A sequence:
    // add r1, r2, r1
    // sltu r3, r1, r2
    // add r4, r3, r5

    let i0 = Rtype::new(insts::OP_ADD, 1, 2, 1);
    let i1 = Rtype::new(insts::OP_SLTU, 3, 1, 2);
    let i2 = Rtype::new(insts::OP_ADD, 4, 3, 5);

    memory.store32(pc, &i0.0).unwrap();
    memory.store32(pc + 4, &i1.0).unwrap();
    memory.store32(pc + 8, &i2.0).unwrap();

    // VERSION1 should NOT fuse ADD3
    let mut decoder_v1 = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);
    let inst_v1 = decoder_v1.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst_v1), insts::OP_ADD);

    // VERSION2 SHOULD fuse ADD3
    let mut decoder_v2 = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION2);
    let inst_v2 = decoder_v2.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst_v2), insts::OP_ADD3A);
}

#[test]
pub fn test_decode_adcs_boundary_registers() {
    let mut memory = SparseMemory::<u64>::default();
    let pc = 0x1000;

    // ADCS sequence:
    // add r0, r1, r2
    // sltu r3, r0, r1

    // Test with x0 (ZERO) - should NOT fuse
    let head_x0 = Rtype::new(insts::OP_ADD, 0, 1, 2);
    let next_x0 = Rtype::new(insts::OP_SLTU, 3, 0, 1);

    memory.store32(pc, &head_x0.0).unwrap();
    memory.store32(pc + 4, &next_x0.0).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION2);
    let inst = decoder.decode(&mut memory, pc).unwrap();
    assert_eq!(extract_opcode(inst), insts::OP_ADD);
}

#[test]
pub fn test_decode_mop_cache_collision() {
    let mut memory = SparseMemory::<u64>::default();

    // PC values that might collide in a 4096-entry cache
    // cache_key = ((pc >> 1 & 0xFF) | (pc >> 1 >> 12 << 8)) % 4096

    let pc1 = 0x1000;
    let pc2 = 0x1000 + (1 << 13); // Should have different key or collision depending on logic

    let head = Rtype::new(insts::OP_ADD, 1, 2, 3);

    memory.store32(pc1, &head.0).unwrap();
    memory.store32(pc2, &head.0).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC | ISA_MOP, VERSION1);

    // Fill cache for pc1
    let _ = decoder.decode(&mut memory, pc1).unwrap();

    // PC2 should not return cached value for PC1
    let inst2 = decoder.decode(&mut memory, pc2).unwrap();
    assert_eq!(extract_opcode(inst2), insts::OP_ADD);
}
