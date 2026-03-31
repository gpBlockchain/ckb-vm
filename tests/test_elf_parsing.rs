use bytes::Bytes;
use ckb_vm::elf::{convert_flags, parse_elf, PF_R, PF_W, PF_X};
use ckb_vm::error::Error;
use ckb_vm::machine::{VERSION0, VERSION2};
use ckb_vm::memory::{FLAG_EXECUTABLE, FLAG_FREEZED};
use ckb_vm::Register;

fn run_parse_elf<R: Register>(
    program: &[u8],
    version: u32,
) -> Result<ckb_vm::elf::ProgramMetadata, Error> {
    parse_elf::<R>(&Bytes::from(program.to_vec()), version)
}

// =========================================================================
// Tests for convert_flags directly (no goblin dependency)
// =========================================================================

#[test]
fn test_convert_flags_executable_only() {
    let flags = PF_R | PF_X;
    let result = convert_flags(flags, false, 0x10000);
    assert!(result.is_ok());
    let f = result.unwrap();
    assert!(f & FLAG_EXECUTABLE != 0, "Should be executable");
    assert!(f & FLAG_FREEZED != 0, "Should be frozen");
}

#[test]
fn test_convert_flags_writable_only_v0() {
    // VERSION0: allow_freeze_writable=true → writable segment GETS frozen
    // (Note: parameter name is misleading; true actually freezes writable segments)
    let flags = PF_R | PF_W;
    let result = convert_flags(flags, true, 0x10000);
    assert!(result.is_ok());
    let f = result.unwrap();
    assert!(f & FLAG_FREEZED != 0, "VERSION0 writable: gets freeze flag");
}

#[test]
fn test_convert_flags_writable_only_v1() {
    // VERSION1+: allow_freeze_writable=false → writable segment does NOT get frozen
    // (Note: parameter name is misleading; false actually leaves writable unfrozen)
    let flags = PF_R | PF_W;
    let result = convert_flags(flags, false, 0x10000);
    assert!(result.is_ok());
    let f = result.unwrap();
    assert!(f & FLAG_FREEZED == 0, "VERSION1+ writable: no freeze flag");
}

#[test]
fn test_convert_flags_readonly() {
    let flags = PF_R;
    let result = convert_flags(flags, false, 0x10000);
    assert!(result.is_ok());
    let f = result.unwrap();
    assert!(f & FLAG_FREEZED != 0, "Read-only should be frozen");
    assert!(
        f & FLAG_EXECUTABLE == 0,
        "Read-only should not be executable"
    );
}

#[test]
fn test_convert_flags_unreadable() {
    // No PF_R flag — should fail
    let flags = PF_W;
    let result = convert_flags(flags, false, 0x10000);
    assert!(result.is_err());
    match result {
        Err(Error::ElfSegmentUnreadable(_)) => {}
        other => panic!("Expected ElfSegmentUnreadable, got: {:?}", other),
    }
}

#[test]
fn test_convert_flags_no_flags_at_all() {
    let result = convert_flags(0, false, 0x10000);
    assert!(result.is_err());
    match result {
        Err(Error::ElfSegmentUnreadable(_)) => {}
        other => panic!("Expected ElfSegmentUnreadable, got: {:?}", other),
    }
}

#[test]
fn test_convert_flags_writable_and_executable() {
    let flags = PF_R | PF_W | PF_X;
    let result = convert_flags(flags, false, 0x10000);
    assert!(result.is_err());
    match result {
        Err(Error::ElfSegmentWritableAndExecutable(_)) => {}
        other => panic!("Expected ElfSegmentWritableAndExecutable, got: {:?}", other),
    }
}

#[test]
fn test_convert_flags_writable_and_executable_vaddr_preserved() {
    let vaddr: u64 = 0xDEAD_BEEF;
    let flags = PF_R | PF_W | PF_X;
    let result = convert_flags(flags, false, vaddr);
    match result {
        Err(Error::ElfSegmentWritableAndExecutable(addr)) => {
            assert_eq!(addr, vaddr, "Error should preserve the vaddr");
        }
        other => panic!("Expected ElfSegmentWritableAndExecutable, got: {:?}", other),
    }
}

#[test]
fn test_convert_flags_unreadable_vaddr_preserved() {
    let vaddr: u64 = 0xCAFE_BABE;
    let result = convert_flags(PF_X, false, vaddr);
    match result {
        Err(Error::ElfSegmentUnreadable(addr)) => {
            assert_eq!(addr, vaddr, "Error should preserve the vaddr");
        }
        other => panic!("Expected ElfSegmentUnreadable, got: {:?}", other),
    }
}

#[test]
fn test_convert_flags_all_valid_combinations() {
    // PF_R only (read-only)
    assert!(convert_flags(PF_R, false, 0).is_ok());
    assert!(convert_flags(PF_R, true, 0).is_ok());
    // PF_R | PF_W (writable)
    assert!(convert_flags(PF_R | PF_W, false, 0).is_ok());
    assert!(convert_flags(PF_R | PF_W, true, 0).is_ok());
    // PF_R | PF_X (executable)
    assert!(convert_flags(PF_R | PF_X, false, 0).is_ok());
    assert!(convert_flags(PF_R | PF_X, true, 0).is_ok());
}

#[test]
fn test_convert_flags_invalid_combinations() {
    // W+X always invalid
    assert!(convert_flags(PF_R | PF_W | PF_X, false, 0).is_err());
    assert!(convert_flags(PF_R | PF_W | PF_X, true, 0).is_err());
    // No read flag
    assert!(convert_flags(PF_W, false, 0).is_err());
    assert!(convert_flags(PF_X, false, 0).is_err());
    assert!(convert_flags(PF_W | PF_X, false, 0).is_err());
    assert!(convert_flags(0, false, 0).is_err());
}

// =========================================================================
// parse_elf with invalid inputs (tests goblin's error handling)
// =========================================================================

#[test]
fn test_parse_elf_empty_input() {
    let result = run_parse_elf::<u64>(&[], VERSION2);
    assert!(result.is_err(), "Empty input should fail");
}

#[test]
fn test_parse_elf_single_byte() {
    let result = run_parse_elf::<u64>(&[0x7f], VERSION2);
    assert!(result.is_err(), "Single byte should fail");
}

#[test]
fn test_parse_elf_partial_magic() {
    let result = run_parse_elf::<u64>(&[0x7f, 0x45, 0x4c], VERSION2);
    assert!(result.is_err(), "Partial ELF magic should fail");
}

#[test]
fn test_parse_elf_wrong_magic() {
    let mut header = vec![0u8; 64];
    header[0] = 0x00;
    header[1] = b'E';
    header[2] = b'L';
    header[3] = b'F';
    let result = run_parse_elf::<u64>(&header, VERSION2);
    assert!(result.is_err(), "Wrong magic should fail");
}

#[test]
fn test_parse_elf_zeroed_header() {
    let header = vec![0u8; 64];
    let result = run_parse_elf::<u64>(&header, VERSION2);
    assert!(result.is_err(), "Zeroed header should fail");
}

#[test]
fn test_parse_elf_truncated_at_16_bytes() {
    let mut header = vec![0u8; 16];
    header[0] = 0x7f;
    header[1] = b'E';
    header[2] = b'L';
    header[3] = b'F';
    let result = run_parse_elf::<u64>(&header, VERSION2);
    assert!(result.is_err(), "16-byte truncated ELF should fail");
}

#[test]
fn test_parse_elf_truncated_at_32_bytes() {
    let mut header = vec![0u8; 32];
    header[0] = 0x7f;
    header[1] = b'E';
    header[2] = b'L';
    header[3] = b'F';
    let result = run_parse_elf::<u64>(&header, VERSION2);
    assert!(result.is_err(), "32-byte truncated ELF should fail");
}

#[test]
fn test_parse_elf_32bit_class_on_64bit_target() {
    let mut elf = vec![0u8; 64];
    elf[0] = 0x7f;
    elf[1] = b'E';
    elf[2] = b'L';
    elf[3] = b'F';
    elf[4] = 1; // 32-bit
    elf[5] = 1; // little-endian
    elf[6] = 1; // version

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    assert!(
        result.is_err(),
        "32-bit ELF with 64-bit register should fail"
    );
    match result {
        Err(Error::ElfBits) => {}
        other => panic!("Expected ElfBits error, got: {:?}", other),
    }
}

#[test]
fn test_parse_elf_invalid_class_value() {
    let mut elf = vec![0u8; 64];
    elf[0] = 0x7f;
    elf[1] = b'E';
    elf[2] = b'L';
    elf[3] = b'F';
    elf[4] = 3; // invalid class
    elf[5] = 1;
    elf[6] = 1;

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    assert!(result.is_err(), "Invalid ELF class should fail");
}

#[test]
fn test_parse_elf_invalid_endian_value() {
    let mut elf = vec![0u8; 64];
    elf[0] = 0x7f;
    elf[1] = b'E';
    elf[2] = b'L';
    elf[3] = b'F';
    elf[4] = 2;
    elf[5] = 3; // invalid endian
    elf[6] = 1;

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    assert!(result.is_err(), "Invalid endianness should fail");
}

#[test]
fn test_parse_elf_valid_header_no_program_headers() {
    let mut elf = vec![0u8; 64];
    elf[0] = 0x7f;
    elf[1] = b'E';
    elf[2] = b'L';
    elf[3] = b'F';
    elf[4] = 2;
    elf[5] = 1;
    elf[6] = 1;
    elf[16] = 2; // ET_EXEC
    elf[18] = 243; // EM_RISCV
    elf[20] = 1;
    elf[24] = 0x00; // e_entry = 0x10000
    elf[25] = 0x00;
    elf[26] = 0x01;
    elf[32] = 64; // e_phoff
    elf[40] = 64; // e_ehsize
    elf[42] = 56; // e_phentsize
    elf[44] = 0; // e_phnum = 0

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    match result {
        Ok(metadata) => {
            assert_eq!(metadata.actions.len(), 0);
            assert_eq!(metadata.entry, 0x10000);
        }
        Err(e) => panic!("Valid header with 0 ph should succeed, got: {:?}", e),
    }
}

#[test]
fn test_parse_elf_max_phnum() {
    let mut elf = vec![0u8; 64];
    elf[0] = 0x7f;
    elf[1] = b'E';
    elf[2] = b'L';
    elf[3] = b'F';
    elf[4] = 2;
    elf[5] = 1;
    elf[6] = 1;
    elf[16] = 2;
    elf[18] = 243;
    elf[20] = 1;
    elf[32] = 64;
    elf[40] = 64;
    elf[42] = 56;
    elf[44..46].copy_from_slice(&u16::MAX.to_le_bytes());

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    // Goblin should handle this — either error or return fewer headers
    // If it succeeds with 0 actions, that's goblin's behavior
    // If it errors, that's also acceptable
    match result {
        Ok(metadata) => {
            // goblin may return 0 headers for insufficient data
            eprintln!("max_phnum: {} actions", metadata.actions.len());
        }
        Err(e) => {
            eprintln!("max_phnum error: {:?}", e);
        }
    }
}

// =========================================================================
// parse_elf using real ELF test binaries
// =========================================================================

#[test]
fn test_parse_elf_real_binary() {
    use std::path::Path;

    let test_bin = "tests/programs/nop64";
    if !Path::new(test_bin).exists() {
        eprintln!("Skipping: test binary {} not found", test_bin);
        return;
    }
    let data = std::fs::read(test_bin).unwrap();
    let result = run_parse_elf::<u64>(&data, VERSION2);
    assert!(result.is_ok(), "Real ELF should parse: {:?}", result.err());
    let metadata = result.unwrap();
    assert!(
        !metadata.actions.is_empty(),
        "Real ELF should have PT_LOAD segments"
    );
    assert!(metadata.entry != 0, "Real ELF should have non-zero entry");
}

#[test]
fn test_parse_elf_real_binary_v0() {
    use std::path::Path;

    let test_bin = "tests/programs/nop64";
    if !Path::new(test_bin).exists() {
        eprintln!("Skipping: test binary {} not found", test_bin);
        return;
    }
    let data = std::fs::read(test_bin).unwrap();
    let result = run_parse_elf::<u64>(&data, VERSION0);
    assert!(
        result.is_ok(),
        "Real ELF v0 should parse: {:?}",
        result.err()
    );
}

#[test]
fn test_parse_elf_truncated_real_binary() {
    use std::path::Path;

    let test_bin = "tests/programs/nop64";
    if !Path::new(test_bin).exists() {
        eprintln!("Skipping: test binary {} not found", test_bin);
        return;
    }
    let data = std::fs::read(test_bin).unwrap();

    // Truncate to just the header
    let truncated = &data[..64.min(data.len())];
    let result = run_parse_elf::<u64>(truncated, VERSION2);
    assert!(result.is_err(), "Truncated ELF should fail");
}

#[test]
fn test_parse_elf_truncated_real_binary_half() {
    use std::path::Path;

    let test_bin = "tests/programs/nop64";
    if !Path::new(test_bin).exists() {
        eprintln!("Skipping: test binary {} not found", test_bin);
        return;
    }
    let data = std::fs::read(test_bin).unwrap();

    // Truncate to half the file — should break segment data
    let half = data.len() / 2;
    let truncated = &data[..half];
    let result = run_parse_elf::<u64>(truncated, VERSION2);
    // Might succeed (if segments fit) or fail (if they don't)
    // Either way, should not panic
    eprintln!("truncated_half: {:?}", result.is_ok());
}

#[test]
fn test_parse_elf_corrupted_real_binary_header() {
    use std::path::Path;

    let test_bin = "tests/programs/nop64";
    if !Path::new(test_bin).exists() {
        eprintln!("Skipping: test binary {} not found", test_bin);
        return;
    }
    let mut data = std::fs::read(test_bin).unwrap();

    // Corrupt the ELF magic
    data[0] = 0x00;
    let result = run_parse_elf::<u64>(&data, VERSION2);
    assert!(result.is_err(), "Corrupted magic should fail");
}

#[test]
fn test_parse_elf_corrupted_phoff() {
    use std::path::Path;

    let test_bin = "tests/programs/nop64";
    if !Path::new(test_bin).exists() {
        eprintln!("Skipping: test binary {} not found", test_bin);
        return;
    }
    let mut data = std::fs::read(test_bin).unwrap();

    // Corrupt e_phoff to a huge value (offset 32-39 for ELF64)
    if data.len() >= 40 {
        data[32..40].copy_from_slice(&999999u64.to_le_bytes());
        let result = run_parse_elf::<u64>(&data, VERSION2);
        // Should either error or return 0 actions (goblin behavior)
        eprintln!("corrupted_phoff: {:?}", result.is_ok());
    }
}

// =========================================================================
// Edge cases for memory calculations in parse_elf
// =========================================================================

#[test]
fn test_parse_elf_big_endian() {
    let mut elf = vec![0u8; 64];
    elf[0] = 0x7f;
    elf[1] = b'E';
    elf[2] = b'L';
    elf[3] = b'F';
    elf[4] = 2; // 64-bit
    elf[5] = 2; // big-endian
    elf[6] = 1;

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    // Should not panic — may succeed or fail depending on goblin
    let _ = result;
}

#[test]
fn test_parse_elf_versions_differ() {
    use std::path::Path;

    let test_bin = "tests/programs/nop64";
    if !Path::new(test_bin).exists() {
        eprintln!("Skipping: test binary {} not found", test_bin);
        return;
    }
    let data = std::fs::read(test_bin).unwrap();

    let result_v0 = run_parse_elf::<u64>(&data, VERSION0);
    let result_v2 = run_parse_elf::<u64>(&data, VERSION2);

    assert!(result_v0.is_ok(), "v0 should parse");
    assert!(result_v2.is_ok(), "v2 should parse");

    let v0 = result_v0.unwrap();
    let v2 = result_v2.unwrap();

    // Both should find same number of actions
    assert_eq!(
        v0.actions.len(),
        v2.actions.len(),
        "Same number of segments"
    );
    assert_eq!(v0.entry, v2.entry, "Same entry point");

    // Flags may differ (VERSION0 writable segments don't get FREEZED)
    for (a0, a2) in v0.actions.iter().zip(v2.actions.iter()) {
        if a0.flags != a2.flags {
            // VERSION0 writable: no freeze, VERSION1+ writable: freeze
            eprintln!(
                "Flag diff at addr {:#x}: v0={:#x} v2={:#x}",
                a0.addr, a0.flags, a2.flags
            );
        }
    }
}
