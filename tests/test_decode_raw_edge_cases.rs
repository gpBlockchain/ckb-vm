use ckb_vm::decoder::{DefaultDecoder, InstDecoder};
use ckb_vm::error::{Error, OutOfBoundKind};
use ckb_vm::machine::VERSION1;
use ckb_vm::memory::Memory;
use ckb_vm::{SparseMemory, ISA_IMC};

#[test]
fn test_decode_raw_pc_equals_memory_size_is_out_of_bound() {
    let mut memory = SparseMemory::<u64>::new(0x10000);
    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC, VERSION1);
    let pc = memory.memory_size() as u64;

    let result = decoder.decode_raw(&mut memory, pc);
    assert!(matches!(
        result,
        Err(Error::MemOutOfBound(addr, OutOfBoundKind::Memory)) if addr == pc
    ));
}

#[test]
fn test_decode_raw_pc_far_out_of_bound_is_out_of_bound() {
    let mut memory = SparseMemory::<u64>::new(0x10000);
    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC, VERSION1);
    let pc = u64::MAX;

    let result = decoder.decode_raw(&mut memory, pc);
    assert!(matches!(
        result,
        Err(Error::MemOutOfBound(addr, OutOfBoundKind::Memory)) if addr == pc
    ));
}

#[test]
fn test_decode_raw_zeroed_memory_returns_invalid_instruction() {
    let mut memory = SparseMemory::<u64>::new(0x10000);
    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC, VERSION1);
    let pc = 0u64;

    let result = decoder.decode_raw(&mut memory, pc);
    assert!(matches!(
        result,
        Err(Error::InvalidInstruction {
            pc: 0,
            instruction: 0
        })
    ));
}

#[test]
fn test_decode_raw_cache_collision_keeps_distinct_pcs() {
    let mut memory = SparseMemory::<u64>::new(0x1000000);
    let pc1 = 0x1000u64;
    let pc2 = pc1 + (1 << 13);
    let i1 = 0x003100b3u64;
    let i2 = 0x003160b3u64;

    memory.store32(&pc1, &i1).unwrap();
    memory.store32(&pc2, &i2).unwrap();

    let mut decoder = DefaultDecoder::new::<u64>(ISA_IMC, VERSION1);
    let d1 = decoder.decode_raw(&mut memory, pc1).unwrap();
    let d2 = decoder.decode_raw(&mut memory, pc2).unwrap();

    assert_ne!(d1, d2);
}
