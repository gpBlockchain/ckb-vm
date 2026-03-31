pub mod machine_build;
use ckb_vm::decoder::{DefaultDecoder, InstDecoder};
use ckb_vm::instructions::extract_opcode;
use ckb_vm::machine::{VERSION1, VERSION2};
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
