use ckb_vm::error::{Error, OutOfBoundKind};

// =========================================================================
// Error variant construction and Display
// =========================================================================

#[test]
fn test_error_asm_display() {
    assert_eq!(Error::Asm(0).to_string(), "asm error: 0");
    assert_eq!(Error::Asm(255).to_string(), "asm error: 255");
}

#[test]
fn test_error_cycles_exceeded_display() {
    assert_eq!(
        Error::CyclesExceeded.to_string(),
        "cycles error: max cycles exceeded"
    );
}

#[test]
fn test_error_cycles_overflow_display() {
    assert_eq!(Error::CyclesOverflow.to_string(), "cycles error: overflow");
}

#[test]
fn test_error_elf_bits_display() {
    assert_eq!(Error::ElfBits.to_string(), "elf error: bits");
}

#[test]
fn test_error_elf_parse_error_display() {
    assert_eq!(
        Error::ElfParseError(String::from("test")).to_string(),
        "elf error: test"
    );
    assert_eq!(
        Error::ElfParseError(String::new()).to_string(),
        "elf error: "
    );
}

#[test]
fn test_error_elf_segment_unreadable_display() {
    assert_eq!(
        Error::ElfSegmentUnreadable(0).to_string(),
        "elf error: segment is unreadable vaddr=0x0"
    );
    assert_eq!(
        Error::ElfSegmentUnreadable(0xDEADBEEF).to_string(),
        "elf error: segment is unreadable vaddr=0xdeadbeef"
    );
    assert_eq!(
        Error::ElfSegmentUnreadable(u64::MAX).to_string(),
        format!("elf error: segment is unreadable vaddr=0x{:x}", u64::MAX)
    );
}

#[test]
fn test_error_elf_writable_executable_display() {
    assert_eq!(
        Error::ElfSegmentWritableAndExecutable(0).to_string(),
        "elf error: segment is writable and executable vaddr=0x0"
    );
    assert_eq!(
        Error::ElfSegmentWritableAndExecutable(0x10000).to_string(),
        "elf error: segment is writable and executable vaddr=0x10000"
    );
}

#[test]
fn test_error_elf_segment_addr_or_size_display() {
    assert_eq!(
        Error::ElfSegmentAddrOrSizeError(0x1234).to_string(),
        "elf error: segment addr or size is wrong vaddr=0x1234"
    );
}

#[test]
fn test_error_external_display() {
    assert_eq!(
        Error::External(String::from("custom error")).to_string(),
        "external error: custom error"
    );
}

#[test]
fn test_error_invalid_ecall_display() {
    assert_eq!(Error::InvalidEcall(42).to_string(), "invalid syscall 42");
}

#[test]
fn test_error_invalid_instruction_display() {
    assert_eq!(
        Error::InvalidInstruction {
            pc: 0x1000,
            instruction: 0x12345678
        }
        .to_string(),
        "invalid instruction pc=0x1000 instruction=0x12345678"
    );
}

#[test]
fn test_error_invalid_instruction_zero() {
    assert_eq!(
        Error::InvalidInstruction {
            pc: 0,
            instruction: 0
        }
        .to_string(),
        "invalid instruction pc=0x0 instruction=0x0"
    );
}

#[test]
fn test_error_invalid_instruction_max() {
    assert_eq!(
        Error::InvalidInstruction {
            pc: u64::MAX,
            instruction: u32::MAX
        }
        .to_string(),
        format!(
            "invalid instruction pc=0x{:x} instruction=0x{:x}",
            u64::MAX,
            u32::MAX
        )
    );
}

#[test]
fn test_error_invalid_op_display() {
    assert_eq!(
        Error::InvalidOp(0xFFFF).to_string(),
        "invalid operand 65535"
    );
    assert_eq!(Error::InvalidOp(0).to_string(), "invalid operand 0");
}

#[test]
fn test_error_invalid_version_display() {
    assert_eq!(Error::InvalidVersion.to_string(), "invalid version");
}

#[test]
fn test_error_io_display() {
    assert_eq!(
        Error::IO {
            kind: std::io::ErrorKind::NotFound,
            data: String::from("file.txt")
        }
        .to_string(),
        "I/O error: NotFound file.txt"
    );
}

#[test]
fn test_error_mem_out_of_bound_display() {
    assert_eq!(
        Error::MemOutOfBound(0x1000, OutOfBoundKind::Memory).to_string(),
        "memory error: out of bound addr=0x1000, kind=Memory"
    );
    assert_eq!(
        Error::MemOutOfBound(0, OutOfBoundKind::ExternalData).to_string(),
        "memory error: out of bound addr=0x0, kind=ExternalData"
    );
}

#[test]
fn test_error_mem_out_of_stack_display() {
    assert_eq!(
        Error::MemOutOfStack.to_string(),
        "memory error: out of stack"
    );
}

#[test]
fn test_error_mem_page_unaligned_display() {
    assert_eq!(
        Error::MemPageUnalignedAccess(0x1234).to_string(),
        "memory error: unaligned page access addr=0x1234"
    );
}

#[test]
fn test_error_mem_write_on_executable_page_display() {
    assert_eq!(
        Error::MemWriteOnExecutablePage(42).to_string(),
        "memory error: write on executable page page_index=42"
    );
}

#[test]
fn test_error_mem_write_on_freezed_page_display() {
    assert_eq!(
        Error::MemWriteOnFreezedPage(100).to_string(),
        "memory error: write on freezed page page_index=100"
    );
}

#[test]
fn test_error_pause_display() {
    assert_eq!(Error::Pause.to_string(), "pause");
}

#[test]
fn test_error_snapshot_data_load_error_display() {
    assert_eq!(
        Error::SnapshotDataLoadError.to_string(),
        "snapshot data load error"
    );
}

#[test]
fn test_error_unexpected_display() {
    assert_eq!(
        Error::Unexpected(String::from("something broke")).to_string(),
        "unexpected error"
    );
}

#[test]
fn test_error_yield_display() {
    assert_eq!(Error::Yield.to_string(), "yield");
}

// =========================================================================
// Error trait implementations
// =========================================================================

#[test]
fn test_error_debug() {
    let e = Error::InvalidInstruction {
        pc: 0x1000,
        instruction: 0x12345678,
    };
    let debug = format!("{:?}", e);
    assert!(debug.contains("InvalidInstruction"));
}

#[test]
fn test_error_clone() {
    let e = Error::MemOutOfBound(0x1000, OutOfBoundKind::Memory);
    let e2 = e.clone();
    assert_eq!(e, e2);
}

#[test]
fn test_error_partial_eq() {
    assert_eq!(Error::CyclesExceeded, Error::CyclesExceeded);
    assert_ne!(Error::CyclesExceeded, Error::CyclesOverflow);
    assert_eq!(Error::Asm(1), Error::Asm(1));
    assert_ne!(Error::Asm(1), Error::Asm(2));
}

#[test]
fn test_error_is_std_error() {
    use std::error::Error as StdError;
    let e = Error::Unexpected(String::from("test"));
    let _: &dyn StdError = &e;
}

// =========================================================================
// Error from conversions
// =========================================================================

#[test]
fn test_error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
    let e: Error = io_err.into();
    match e {
        Error::IO { kind, data } => {
            assert_eq!(kind, std::io::ErrorKind::PermissionDenied);
            assert!(data.contains("access denied"));
        }
        _ => panic!("Expected IO error"),
    }
}

#[test]
fn test_error_from_io_error_not_found() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
    let e: Error = io_err.into();
    match e {
        Error::IO { kind, .. } => {
            assert_eq!(kind, std::io::ErrorKind::NotFound);
        }
        _ => panic!("Expected IO error"),
    }
}

// =========================================================================
// OutOfBoundKind
// =========================================================================

#[test]
fn test_out_of_bound_kind_debug() {
    assert_eq!(format!("{:?}", OutOfBoundKind::Memory), "Memory");
    assert_eq!(
        format!("{:?}", OutOfBoundKind::ExternalData),
        "ExternalData"
    );
}

#[test]
fn test_out_of_bound_kind_clone() {
    let k = OutOfBoundKind::Memory;
    let k2 = k.clone();
    assert_eq!(k, k2);
}

// =========================================================================
// Error equality across all variants
// =========================================================================

#[test]
fn test_all_error_variants_constructable() {
    // Ensure all variants can be constructed without panic
    let _ = Error::Asm(0);
    let _ = Error::CyclesExceeded;
    let _ = Error::CyclesOverflow;
    let _ = Error::ElfBits;
    let _ = Error::ElfParseError(String::new());
    let _ = Error::ElfSegmentUnreadable(0);
    let _ = Error::ElfSegmentWritableAndExecutable(0);
    let _ = Error::ElfSegmentAddrOrSizeError(0);
    let _ = Error::External(String::new());
    let _ = Error::InvalidEcall(0);
    let _ = Error::InvalidInstruction {
        pc: 0,
        instruction: 0,
    };
    let _ = Error::InvalidOp(0);
    let _ = Error::InvalidVersion;
    let _ = Error::IO {
        kind: std::io::ErrorKind::Other,
        data: String::new(),
    };
    let _ = Error::MemOutOfBound(0, OutOfBoundKind::Memory);
    let _ = Error::MemOutOfStack;
    let _ = Error::MemPageUnalignedAccess(0);
    let _ = Error::MemWriteOnExecutablePage(0);
    let _ = Error::MemWriteOnFreezedPage(0);
    let _ = Error::Pause;
    let _ = Error::SnapshotDataLoadError;
    let _ = Error::Unexpected(String::new());
    let _ = Error::Yield;
}
