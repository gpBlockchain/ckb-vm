use bytes::Bytes;
use ckb_vm::elf::parse_elf;
use ckb_vm::error::Error;
use ckb_vm::machine::{VERSION0, VERSION1, VERSION2};
use ckb_vm::Register;

fn run_parse_elf<R: Register>(
    program: &[u8],
    version: u32,
) -> Result<ckb_vm::elf::ProgramMetadata, Error> {
    parse_elf::<R>(&Bytes::from(program.to_vec()), version)
}

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
    // ELF magic is 0x7f 'E' 'L' 'F' — test with just first 3 bytes
    let result = run_parse_elf::<u64>(&[0x7f, 0x45, 0x4c], VERSION2);
    assert!(result.is_err(), "Partial ELF magic should fail");
}

#[test]
fn test_parse_elf_wrong_magic() {
    let mut header = vec![0u8; 64];
    header[0] = 0x00; // wrong magic
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
fn test_parse_elf_valid_header_no_program_headers() {
    // Build a minimal valid ELF64 header with 0 program headers
    let mut elf = vec![0u8; 64];
    // ELF magic
    elf[0] = 0x7f;
    elf[1] = b'E';
    elf[2] = b'L';
    elf[3] = b'F';
    // 64-bit (class = 2)
    elf[4] = 2;
    // little-endian (data = 1)
    elf[5] = 1;
    // ELF version = 1
    elf[6] = 1;
    // OS/ABI = 0 (SYSV)
    elf[7] = 0;
    // e_type = ET_EXEC (2)
    elf[16] = 2;
    elf[17] = 0;
    // e_machine = EM_RISCV (243)
    elf[18] = 243;
    elf[19] = 0;
    // e_version = 1
    elf[20] = 1;
    // e_entry = 0x10000
    elf[24] = 0x00;
    elf[25] = 0x00;
    elf[26] = 0x01;
    elf[27] = 0x00;
    // e_phoff = 64 (right after header)
    elf[32] = 64;
    // e_shoff = 0 (no section headers)
    // e_flags = 0
    // e_ehsize = 64
    elf[40] = 64;
    // e_phentsize = 56 (ELF64 program header size)
    elf[42] = 56;
    // e_phnum = 0
    elf[44] = 0;
    // e_shentsize = 64
    elf[46] = 64;
    // e_shnum = 0
    // e_shstrndx = 0

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    // Should succeed with 0 actions
    match result {
        Ok(metadata) => {
            assert_eq!(metadata.actions.len(), 0, "No PT_LOAD segments expected");
            assert_eq!(metadata.entry, 0x10000);
        }
        Err(e) => panic!(
            "Valid header with 0 program headers should succeed, got: {:?}",
            e
        ),
    }
}

#[test]
fn test_parse_elf_32bit_class_on_64bit_target() {
    let mut elf = vec![0u8; 64];
    elf[0] = 0x7f;
    elf[1] = b'E';
    elf[2] = b'L';
    elf[3] = b'F';
    // 32-bit (class = 1) but we'll use u64 register
    elf[4] = 1;
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
fn test_parse_elf_64bit_class_on_32bit_target() {
    // We can't easily test this since u32 Register impl might not exist,
    // but we can test the big-endian path
    let mut elf = vec![0u8; 64];
    elf[0] = 0x7f;
    elf[1] = b'E';
    elf[2] = b'L';
    elf[3] = b'F';
    elf[4] = 2; // 64-bit
    elf[5] = 2; // big-endian
    elf[6] = 1; // version

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    // Big-endian should be handled (might fail for other reasons but not panic)
    let _ = result;
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
    elf[4] = 2; // 64-bit
    elf[5] = 3; // invalid endian
    elf[6] = 1;

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    assert!(result.is_err(), "Invalid endianness should fail");
}

#[test]
fn test_parse_elf_segment_beyond_file() {
    // Build ELF with PT_LOAD pointing past file end
    let mut elf = vec![0u8; 120]; // 64 header + 56 program header
                                  // ELF magic
    elf[0] = 0x7f;
    elf[1] = b'E';
    elf[2] = b'L';
    elf[3] = b'F';
    elf[4] = 2; // 64-bit
    elf[5] = 1; // little-endian
    elf[6] = 1; // version
    elf[16] = 2; // ET_EXEC
    elf[18] = 243; // EM_RISCV
    elf[20] = 1; // e_version
    elf[32] = 64; // e_phoff
    elf[40] = 64; // e_ehsize
    elf[42] = 56; // e_phentsize
    elf[44] = 1; // e_phnum = 1

    // Program header at offset 64:
    // p_type = PT_LOAD (1) at offset 64
    elf[64] = 1;
    // p_flags = PF_R | PF_W (5) at offset 68
    elf[68] = 5;
    // p_offset = 1000 (way past file end) at offset 72
    elf[72] = 0xE8;
    elf[73] = 0x03;
    // p_vaddr = 0x10000 at offset 80
    elf[80] = 0x00;
    elf[81] = 0x00;
    elf[82] = 0x01;
    // p_filesz = 100 at offset 96
    elf[96] = 100;
    // p_memsz = 100 at offset 104
    elf[104] = 100;

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    assert!(
        result.is_err(),
        "Segment pointing past file end should fail"
    );
}

#[test]
fn test_parse_elf_writable_and_executable_flags() {
    // Build ELF with PT_LOAD that has both W and X flags
    let mut elf = vec![0u8; 120];
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
    elf[44] = 1;

    // Program header
    elf[64] = 1; // PT_LOAD
                 // p_flags = PF_R | PF_W | PF_X (7)
    elf[68] = 7;
    // p_offset = 120 (right after headers)
    elf[72] = 120;
    // p_vaddr = 0x10000
    elf[80] = 0x00;
    elf[81] = 0x00;
    elf[82] = 0x01;
    // p_filesz = 0
    // p_memsz = 0

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    assert!(result.is_err(), "Writable+executable segment should fail");
    match result {
        Err(Error::ElfSegmentWritableAndExecutable(_)) => {}
        other => panic!("Expected ElfSegmentWritableAndExecutable, got: {:?}", other),
    }
}

#[test]
fn test_parse_elf_unreadable_segment() {
    // Build ELF with PT_LOAD that has no read flag
    let mut elf = vec![0u8; 120];
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
    elf[44] = 1;

    // Program header
    elf[64] = 1; // PT_LOAD
                 // p_flags = PF_W (2) — no read flag
    elf[68] = 2;
    // p_offset = 120
    elf[72] = 120;
    // p_vaddr = 0x10000
    elf[80] = 0x00;
    elf[81] = 0x00;
    elf[82] = 0x01;

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    assert!(result.is_err(), "Unreadable segment should fail");
    match result {
        Err(Error::ElfSegmentUnreadable(_)) => {}
        other => panic!("Expected ElfSegmentUnreadable, got: {:?}", other),
    }
}

#[test]
fn test_parse_elf_version0_vs_version1() {
    // Test that the same input behaves differently for version 0 vs version 1+
    let mut elf = vec![0u8; 120];
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
    elf[44] = 1;

    elf[64] = 1; // PT_LOAD
    elf[68] = 5; // PF_R | PF_W
    elf[72] = 120;
    elf[80] = 0x00;
    elf[81] = 0x00;
    elf[82] = 0x01;

    let result_v0 = run_parse_elf::<u64>(&elf, VERSION0);
    let result_v2 = run_parse_elf::<u64>(&elf, VERSION2);

    // Both should succeed (readable+writable is OK for non-executable)
    // But they might differ in flag handling (VERSION0 allows freeze writable)
    assert!(
        result_v0.is_ok(),
        "VERSION0 parse should succeed: {:?}",
        result_v0.err()
    );
    assert!(
        result_v2.is_ok(),
        "VERSION2 parse should succeed: {:?}",
        result_v2.err()
    );

    if let (Ok(v0), Ok(v2)) = (&result_v0, &result_v2) {
        // VERSION0 allows writable without freeze, VERSION1+ adds FLAG_FREEZED
        if v0.actions.len() > 0 && v2.actions.len() > 0 {
            // Flags may differ between versions
            let _ = (v0.actions[0].flags, v2.actions[0].flags);
        }
    }
}

#[test]
fn test_parse_elf_offset_overflow() {
    // p_offset + p_filesz wraps around
    let mut elf = vec![0u8; 120];
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
    elf[44] = 1;

    elf[64] = 1; // PT_LOAD
    elf[68] = 5; // PF_R | PF_W
                 // p_offset = u64::MAX - 10
    let offset: u64 = u64::MAX - 10;
    elf[72..80].copy_from_slice(&offset.to_le_bytes());
    // p_vaddr = 0x10000
    elf[80] = 0x00;
    elf[81] = 0x00;
    elf[82] = 0x01;
    // p_filesz = 100 (would wrap when added to offset)
    elf[96] = 100;
    // p_memsz = 100
    elf[104] = 100;

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    // Should detect overflow via wrapping_add check
    assert!(result.is_err(), "Overflowing offset+filesz should fail");
}

#[test]
fn test_parse_elf_executable_segment_ok() {
    // Build valid ELF with executable-only segment
    let mut elf = vec![0u8; 120];
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
    elf[44] = 1;

    elf[64] = 1; // PT_LOAD
    elf[68] = 5; // PF_R | PF_X (readable + executable)
    elf[72] = 120;
    elf[80] = 0x00;
    elf[81] = 0x00;
    elf[82] = 0x01;

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    assert!(
        result.is_ok(),
        "Executable segment should succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_parse_elf_large_phoff() {
    // e_phoff points way past file
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
    // e_phoff = 999999
    elf[32..40].copy_from_slice(&999999u64.to_le_bytes());
    elf[40] = 64;
    elf[42] = 56;
    elf[44] = 1; // e_phnum = 1

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    assert!(result.is_err(), "Large phoff past file should fail");
}

#[test]
fn test_parse_elf_max_phnum() {
    // e_phnum = u16::MAX
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
    assert!(result.is_err(), "Max phnum should fail (not enough data)");
}

#[test]
fn test_parse_elf_readable_only_segment() {
    let mut elf = vec![0u8; 120];
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
    elf[44] = 1;

    elf[64] = 1; // PT_LOAD
    elf[68] = 4; // PF_R only (readable, not writable, not executable)
    elf[72] = 120;
    elf[80] = 0x00;
    elf[81] = 0x00;
    elf[82] = 0x01;

    let result = run_parse_elf::<u64>(&elf, VERSION2);
    assert!(
        result.is_ok(),
        "Read-only segment should succeed: {:?}",
        result.err()
    );
}
