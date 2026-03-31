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
