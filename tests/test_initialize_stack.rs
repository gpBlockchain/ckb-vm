use bytes::Bytes;
use ckb_vm::cost_model::constant_cycles;
use ckb_vm::machine::{
    CoreMachine, DefaultCoreMachine, SupportMachine, VERSION0, VERSION1, VERSION2,
};
use ckb_vm::registers::SP;
use ckb_vm::{
    Error, Register, RustDefaultMachineBuilder, SparseMemory, WXorXMemory, DEFAULT_MEMORY_SIZE,
    ISA_IMC, ISA_MOP,
};

fn make_machine(version: u32) -> impl SupportMachine {
    let core = DefaultCoreMachine::<u64, WXorXMemory<SparseMemory<u64>>>::new(
        ISA_IMC | ISA_MOP,
        version,
        u64::MAX,
    );
    RustDefaultMachineBuilder::<DefaultCoreMachine<u64, WXorXMemory<SparseMemory<u64>>>>::new(core)
        .instruction_cycle_func(Box::new(constant_cycles))
        .build()
}

fn stack_params() -> (u64, u64) {
    let memory_size = DEFAULT_MEMORY_SIZE as u64;
    let stack_size = memory_size / 4;
    let stack_start = memory_size - stack_size;
    (stack_start, stack_size)
}

#[test]
fn test_initialize_stack_empty_args_version0() {
    let mut machine = make_machine(VERSION0);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    let (stack_start, stack_size) = stack_params();
    let result = machine.initialize_stack([].into_iter(), stack_start, stack_size);
    assert!(result.is_ok());
    // VERSION0 does not skip stack writing for empty args — argc (8 bytes) is still written
    let used = result.unwrap();
    assert_eq!(used, 8);
}

#[test]
fn test_initialize_stack_empty_args_version1() {
    let mut machine = make_machine(VERSION1);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    let (stack_start, stack_size) = stack_params();
    let result = machine.initialize_stack([].into_iter(), stack_start, stack_size);
    assert!(result.is_ok());
    // VERSION1 with empty args: sets argc to 0, aligns SP to 16 bytes
    let sp = machine.registers()[SP].to_u64();
    assert!(sp.is_multiple_of(16));
    assert!(sp >= stack_start);
    assert!(sp <= stack_start + stack_size);
}

#[test]
fn test_initialize_stack_empty_args_version2() {
    let mut machine = make_machine(VERSION2);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    let (stack_start, stack_size) = stack_params();
    let result = machine.initialize_stack([].into_iter(), stack_start, stack_size);
    assert!(result.is_ok());
    let sp = machine.registers()[SP].to_u64();
    assert!(sp.is_multiple_of(16));
    assert!(sp >= stack_start);
}

#[test]
fn test_initialize_stack_single_empty_arg() {
    let mut machine = make_machine(VERSION1);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    let (stack_start, stack_size) = stack_params();
    // A single empty Bytes argument
    let result = machine.initialize_stack([Ok(Bytes::new())].into_iter(), stack_start, stack_size);
    assert!(result.is_ok());
    let sp = machine.registers()[SP].to_u64();
    assert!(sp.is_multiple_of(16));
    assert!(sp >= stack_start);
}

#[test]
fn test_initialize_stack_multiple_empty_args() {
    let mut machine = make_machine(VERSION1);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    let (stack_start, stack_size) = stack_params();
    // Multiple empty arguments
    let result = machine.initialize_stack(
        [Ok(Bytes::new()), Ok(Bytes::new()), Ok(Bytes::new())].into_iter(),
        stack_start,
        stack_size,
    );
    assert!(result.is_ok());
    let sp = machine.registers()[SP].to_u64();
    assert!(sp.is_multiple_of(16));
    assert!(sp >= stack_start);
}

#[test]
#[should_panic(expected = "attempt to subtract with overflow")]
fn test_initialize_stack_zero_stack_size() {
    // BUG: integer underflow panic when stack_size=0 and version >= 1 with empty args
    // origin_sp (stack_start + 0 = stack_start) - argc_size (8) underflows
    let mut machine = make_machine(VERSION1);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    let result = machine.initialize_stack([].into_iter(), 0, 0);
    assert!(result.is_ok());
}

#[test]
fn test_initialize_stack_zero_stack_size_with_args() {
    let mut machine = make_machine(VERSION1);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    let result = machine.initialize_stack([Ok(Bytes::from_static(b"hello"))].into_iter(), 0, 0);
    // With zero stack size, should fail with out of stack
    assert!(result.is_err());
}

#[test]
fn test_initialize_stack_one_byte_stack() {
    let mut machine = make_machine(VERSION1);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    // Stack of 1 byte — far too small for any args
    let result = machine.initialize_stack(
        [Ok(Bytes::from_static(b"hello"))].into_iter(),
        1024 * 1024,
        1,
    );
    assert!(result.is_err());
}

#[test]
fn test_initialize_stack_single_large_arg() {
    let mut machine = make_machine(VERSION1);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    let (stack_start, stack_size) = stack_params();
    // Arg larger than stack
    let large_arg = Bytes::from(vec![0x41u8; (stack_size as usize) + 1]);
    let result = machine.initialize_stack([Ok(large_arg)].into_iter(), stack_start, stack_size);
    assert!(result.is_err());
}

#[test]
fn test_initialize_stack_arg_exactly_fills_stack() {
    let mut machine = make_machine(VERSION1);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    let (stack_start, stack_size) = stack_params();
    // Arg that fits exactly: leave room for argc + alignment
    let max_arg_size = stack_size as usize - 64;
    let arg = Bytes::from(vec![0x42u8; max_arg_size]);
    let result = machine.initialize_stack([Ok(arg)].into_iter(), stack_start, stack_size);
    assert!(result.is_ok());
    let sp = machine.registers()[SP].to_u64();
    assert!(sp >= stack_start);
}

#[test]
fn test_initialize_stack_error_in_arg_iterator() {
    let mut machine = make_machine(VERSION1);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    let (stack_start, stack_size) = stack_params();
    // First arg is an error
    let result = machine.initialize_stack(
        [Err(Error::Unexpected(String::from("test error")))].into_iter(),
        stack_start,
        stack_size,
    );
    assert!(result.is_err());
}

#[test]
fn test_initialize_stack_error_after_valid_arg() {
    let mut machine = make_machine(VERSION1);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    let (stack_start, stack_size) = stack_params();
    // Second arg is an error
    let result = machine.initialize_stack(
        [
            Ok(Bytes::from_static(b"ok")),
            Err(Error::Unexpected(String::from("test error"))),
        ]
        .into_iter(),
        stack_start,
        stack_size,
    );
    assert!(result.is_err());
}

#[test]
fn test_initialize_stack_multiple_args_v0() {
    let mut machine = make_machine(VERSION0);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    let (stack_start, stack_size) = stack_params();
    let result = machine.initialize_stack(
        [
            Ok(Bytes::from_static(b"hello")),
            Ok(Bytes::from_static(b"world")),
        ]
        .into_iter(),
        stack_start,
        stack_size,
    );
    assert!(result.is_ok());
}

#[test]
fn test_initialize_stack_sp_never_below_stack_start() {
    for version in &[VERSION0, VERSION1, VERSION2] {
        let mut machine = make_machine(*version);
        let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
        machine.load_elf(&code, true).unwrap();

        let (stack_start, stack_size) = stack_params();
        let _ = machine.initialize_stack(
            [Ok(Bytes::from_static(b"tiny"))].into_iter(),
            stack_start,
            stack_size,
        );
        let sp = machine.registers()[SP].to_u64();
        assert!(
            sp >= stack_start,
            "SP 0x{:x} below stack_start 0x{:x} for version {}",
            sp,
            stack_start,
            version
        );
    }
}

#[test]
fn test_initialize_stack_version2_early_exit_on_overflow() {
    let mut machine = make_machine(VERSION2);
    let code = Bytes::from(std::fs::read("tests/programs/simple64").unwrap());
    machine.load_elf(&code, true).unwrap();

    // Small stack, large args — VERSION2 should exit early when SP < stack_start
    let stack_start = 1024 * 1024;
    let stack_size = 1024;
    let result = machine.initialize_stack(
        [Ok(Bytes::from(vec![0u8; 2048]))].into_iter(),
        stack_start,
        stack_size,
    );
    assert!(result.is_err());
}
