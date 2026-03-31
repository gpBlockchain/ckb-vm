use bytes::Bytes;
use ckb_vm::memory::{FLAG_EXECUTABLE, FLAG_FREEZED, FLAG_WRITABLE, FLAG_WXORX_BIT};
use ckb_vm::{Memory, Register, SparseMemory, WXorXMemory, DEFAULT_MEMORY_SIZE};
use ckb_vm_definitions::RISCV_PAGESIZE;

type Mem = WXorXMemory<SparseMemory<u64>>;

#[test]
fn test_wxorx_store_to_executable_page_fails() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    mem.init_pages(
        0x10000,
        RISCV_PAGESIZE as u64,
        FLAG_EXECUTABLE | FLAG_FREEZED,
        None,
        0,
    )
    .unwrap();
    let result = mem.store64(&0x10000u64, &42u64);
    assert!(result.is_err(), "Store to executable page should fail");
}

#[test]
fn test_wxorx_execute_load_from_writable_fails() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    mem.init_pages(0x10000, RISCV_PAGESIZE as u64, FLAG_WRITABLE, None, 0)
        .unwrap();
    let result = mem.execute_load32(0x10000);
    assert!(result.is_err(), "Execute from writable page should fail");
}

#[test]
fn test_wxorx_store_bytes_empty_skips_check() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    // No init — but empty store should still succeed
    let result = mem.store_bytes(0x10000, &[]);
    assert!(result.is_ok(), "Empty store_bytes should succeed");
}

#[test]
fn test_wxorx_store_byte_zero_size_skips_check() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    let result = mem.store_byte(0x10000, 0, 0xFF);
    assert!(result.is_ok(), "store_byte with size 0 should succeed");
}

#[test]
fn test_wxorx_init_pages_exact_memory_boundary() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    let last_page = (DEFAULT_MEMORY_SIZE - RISCV_PAGESIZE) as u64;
    let result = mem.init_pages(last_page, RISCV_PAGESIZE as u64, FLAG_WRITABLE, None, 0);
    assert!(result.is_ok(), "Init at last page should succeed");
}

#[test]
fn test_wxorx_init_pages_one_past_memory() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    let result = mem.init_pages(
        DEFAULT_MEMORY_SIZE as u64,
        RISCV_PAGESIZE as u64,
        FLAG_WRITABLE,
        None,
        0,
    );
    assert!(result.is_err(), "Init past memory should fail");
}

#[test]
fn test_wxorx_init_pages_with_source_data() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    let data = vec![0xABu8; RISCV_PAGESIZE];
    mem.init_pages(
        0x10000,
        RISCV_PAGESIZE as u64,
        FLAG_WRITABLE,
        Some(Bytes::from(data.clone())),
        0,
    )
    .unwrap();
    let loaded = mem.load_bytes(0x10000, RISCV_PAGESIZE as u64).unwrap();
    assert_eq!(&loaded[..], &data[..]);
}

#[test]
fn test_wxorx_init_pages_source_with_offset() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    let data = vec![0x11, 0x22, 0x33, 0x44];
    mem.init_pages(
        0x10000,
        RISCV_PAGESIZE as u64,
        FLAG_WRITABLE,
        Some(Bytes::from(data)),
        2,
    )
    .unwrap();
    let loaded = mem.load_bytes(0x10000, 6).unwrap();
    assert_eq!(loaded[0], 0);
    assert_eq!(loaded[1], 0);
    assert_eq!(loaded[2], 0x11);
    assert_eq!(loaded[3], 0x22);
}

#[test]
fn test_wxorx_store64_straddling_page_boundary() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    mem.init_pages(0x10000, 2 * RISCV_PAGESIZE as u64, FLAG_WRITABLE, None, 0)
        .unwrap();
    // Store 8 bytes straddling page boundary
    let addr = 0x10000 + RISCV_PAGESIZE as u64 - 4;
    let result = mem.store64(&addr, &0xDEADBEEFu64);
    assert!(result.is_ok());
    let loaded = mem.load64(&addr).unwrap().to_u64();
    assert_eq!(loaded, 0xDEADBEEF);
}

#[test]
fn test_wxorx_store64_straddling_writable_executable_fails() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    mem.init_pages(0x10000, RISCV_PAGESIZE as u64, FLAG_WRITABLE, None, 0)
        .unwrap();
    mem.init_pages(
        0x10000 + RISCV_PAGESIZE as u64,
        RISCV_PAGESIZE as u64,
        FLAG_EXECUTABLE | FLAG_FREEZED,
        None,
        0,
    )
    .unwrap();
    let addr = 0x10000 + RISCV_PAGESIZE as u64 - 4;
    let result = mem.store64(&addr, &0x1234u64);
    assert!(
        result.is_err(),
        "Store straddling writable+executable should fail"
    );
}

#[test]
fn test_wxorx_init_pages_multiple_pages_flag_check() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    let pages = 10usize;
    let size = pages * RISCV_PAGESIZE;
    let start_page = 0x10000u64 / RISCV_PAGESIZE as u64;
    mem.init_pages(0x10000, size as u64, FLAG_WRITABLE, None, 0)
        .unwrap();
    for i in 0..pages as u64 {
        let page = start_page + i;
        let flag = mem.fetch_flag(page).unwrap();
        // FLAG_WRITABLE is 0; writable means FLAG_WXORX_BIT is NOT set
        assert!(
            flag & FLAG_WXORX_BIT == 0,
            "Page {} (flag=0x{:x}) should be writable (WXORX_BIT clear)",
            page,
            flag
        );
    }
}

#[test]
fn test_wxorx_reinit_non_frozen_page() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    // Init as writable (no freeze)
    mem.init_pages(0x10000, RISCV_PAGESIZE as u64, FLAG_WRITABLE, None, 0)
        .unwrap();
    // Re-init as executable — should succeed since not frozen
    let result = mem.init_pages(0x10000, RISCV_PAGESIZE as u64, FLAG_EXECUTABLE, None, 0);
    assert!(result.is_ok(), "Re-init non-frozen page should succeed");
}

#[test]
fn test_wxorx_addr_plus_size_wrapping() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    // addr near u64::MAX + size wraps
    let result = mem.init_pages(
        u64::MAX - RISCV_PAGESIZE as u64 + 1,
        RISCV_PAGESIZE as u64,
        FLAG_WRITABLE,
        None,
        0,
    );
    assert!(result.is_err(), "Wrapping addr+size should fail");
}

#[test]
fn test_wxorx_offset_exactly_equals_size() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    let size = RISCV_PAGESIZE as u64;
    // offset == size is allowed
    let result = mem.init_pages(0x10000, size, FLAG_WRITABLE, None, size);
    assert!(result.is_ok(), "offset == size should be OK");
}

#[test]
fn test_wxorx_zero_size_no_page_init() {
    let mut mem = Mem::new(DEFAULT_MEMORY_SIZE);
    mem.init_pages(0x10000, 0, FLAG_WRITABLE, None, 0).unwrap();
    // No pages should be affected
    let flag = mem.fetch_flag(0x10000 / RISCV_PAGESIZE as u64).unwrap();
    assert_eq!(flag, 0, "Zero-size init should not set any flags");
}
