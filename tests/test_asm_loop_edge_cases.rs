use ckb_vm::machine::asm::{AsmCoreMachine, AsmMachine, AsmDefaultMachineBuilder};
use ckb_vm::machine::{VERSION1};
use ckb_vm::{Error, ISA_IMC, ISA_MOP, DefaultMachineRunner, SupportMachine};
use ckb_vm::decoder::InstDecoder;
use ckb_vm::machine::asm::traces::SimpleFixedTraceDecoder;

#[test]
pub fn test_asm_run_with_invalid_version_mop() {
    let mut decoder = SimpleFixedTraceDecoder::new::<u64>(ISA_IMC | ISA_MOP, 0);
    let core = AsmCoreMachine::new(ISA_IMC | ISA_MOP, 0, u64::MAX);
    let mut machine = AsmMachine::new(AsmDefaultMachineBuilder::new(core).build());
    
    // VERSION0 (0) with ISA_MOP should return InvalidVersion
    let res = machine.run_with_decoder(&mut decoder);
    assert!(matches!(res, Err(Error::InvalidVersion)));
}

#[test]
pub fn test_asm_step_empty_program() {
    let mut decoder = SimpleFixedTraceDecoder::new::<u64>(ISA_IMC, VERSION1);
    let core = AsmCoreMachine::new(ISA_IMC, VERSION1, u64::MAX);
    let mut machine = AsmMachine::new(AsmDefaultMachineBuilder::new(core).build());
    
    // Step on uninitialized machine (PC=0)
    let res = machine.step(&mut decoder);
    assert!(res.is_err());
}
