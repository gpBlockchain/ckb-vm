use ckb_vm::machine::{DefaultCoreMachine, SupportMachine, VERSION1};
use ckb_vm::{Error, SparseMemory, WXorXMemory, ISA_IMC, ISA_MOP};

type Core = DefaultCoreMachine<u64, WXorXMemory<SparseMemory<u64>>>;

#[test]
fn test_add_cycles_at_limit() {
    let mut machine = Core::new(ISA_IMC | ISA_MOP, VERSION1, 100);
    machine.set_cycles(99);
    assert!(machine.add_cycles(1).is_ok());
    assert_eq!(machine.cycles(), 100);
}

#[test]
fn test_add_cycles_one_below_limit() {
    let mut machine = Core::new(ISA_IMC | ISA_MOP, VERSION1, 100);
    machine.set_cycles(99);
    assert!(machine.add_cycles(2).is_err());
}

#[test]
fn test_add_cycles_exactly_exceeds() {
    let mut machine = Core::new(ISA_IMC | ISA_MOP, VERSION1, 100);
    machine.set_cycles(100);
    assert_eq!(machine.add_cycles(1), Err(Error::CyclesExceeded));
}

#[test]
fn test_add_cycles_zero() {
    let mut machine = Core::new(ISA_IMC | ISA_MOP, VERSION1, 100);
    machine.set_cycles(50);
    assert!(machine.add_cycles(0).is_ok());
    assert_eq!(machine.cycles(), 50);
}

#[test]
fn test_add_cycles_overflow() {
    let mut machine = Core::new(ISA_IMC | ISA_MOP, VERSION1, u64::MAX);
    machine.set_cycles(u64::MAX);
    assert_eq!(machine.add_cycles(1), Err(Error::CyclesOverflow));
}

#[test]
fn test_add_cycles_overflow_before_max_check() {
    let mut machine = Core::new(ISA_IMC | ISA_MOP, VERSION1, 100);
    machine.set_cycles(u64::MAX - 1);
    // Overflow happens before CyclesExceeded check
    assert_eq!(machine.add_cycles(5), Err(Error::CyclesOverflow));
}

#[test]
fn test_add_cycles_no_checking_at_limit() {
    let mut machine = Core::new(ISA_IMC | ISA_MOP, VERSION1, 100);
    machine.set_cycles(99);
    assert!(machine.add_cycles_no_checking(1).is_ok());
    assert_eq!(machine.cycles(), 100);
}

#[test]
fn test_add_cycles_no_checking_exceeds_max() {
    let mut machine = Core::new(ISA_IMC | ISA_MOP, VERSION1, 100);
    machine.set_cycles(99);
    // No checking against max — should succeed
    assert!(machine.add_cycles_no_checking(200).is_ok());
    assert_eq!(machine.cycles(), 299);
}

#[test]
fn test_add_cycles_no_checking_overflow() {
    let mut machine = Core::new(ISA_IMC | ISA_MOP, VERSION1, u64::MAX);
    machine.set_cycles(u64::MAX);
    assert_eq!(
        machine.add_cycles_no_checking(1),
        Err(Error::CyclesOverflow)
    );
}

#[test]
fn test_add_cycles_exact_boundary() {
    let mut machine = Core::new(ISA_IMC | ISA_MOP, VERSION1, 1000);
    machine.set_cycles(0);
    assert!(machine.add_cycles(1000).is_ok());
    assert_eq!(machine.cycles(), 1000);
}

#[test]
fn test_add_cycles_one_over_boundary() {
    let mut machine = Core::new(ISA_IMC | ISA_MOP, VERSION1, 1000);
    machine.set_cycles(0);
    assert_eq!(machine.add_cycles(1001), Err(Error::CyclesExceeded));
}

#[test]
fn test_add_cycles_max_cycles_zero() {
    let mut machine = Core::new(ISA_IMC | ISA_MOP, VERSION1, 0);
    machine.set_cycles(0);
    // Adding 0 should succeed even with max_cycles=0
    assert!(machine.add_cycles(0).is_ok());
    // Adding 1 should fail
    assert_eq!(machine.add_cycles(1), Err(Error::CyclesExceeded));
}

#[test]
fn test_add_cycles_accumulates() {
    let mut machine = Core::new(ISA_IMC | ISA_MOP, VERSION1, 100);
    machine.set_cycles(0);
    for _ in 0..50 {
        assert!(machine.add_cycles(2).is_ok());
    }
    assert_eq!(machine.cycles(), 100);
    // One more should fail
    assert_eq!(machine.add_cycles(1), Err(Error::CyclesExceeded));
}
