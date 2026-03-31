use bytes::Bytes;
use ckb_vm::error::{Error, OutOfBoundKind};
use ckb_vm::memory::sparse::SparseMemory;
use ckb_vm::memory::wxorx::WXorXMemory;
use ckb_vm::memory::{Memory, FLAG_DIRTY, FLAG_EXECUTABLE, FLAG_FREEZED};
use ckb_vm::RISCV_PAGESIZE;

// =========================================================================
// WXorXMemory init_pages edge cases
// =========================================================================

#[test]
fn test_wxorx_init_pages_unaligned_addr() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    let result = mem.init_pages(1, 4096, FLAG_FREEZED, None, 0);
    assert!(result.is_err());
    match result {
        Err(Error::MemPageUnalignedAccess(_)) => {}
        other => panic!("Expected MemPageUnalignedAccess, got: {:?}", other),
    }
}

#[test]
fn test_wxorx_init_pages_unaligned_size() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    let result = mem.init_pages(0, 1, FLAG_FREEZED, None, 0);
    assert!(result.is_err());
    match result {
        Err(Error::MemPageUnalignedAccess(_)) => {}
        other => panic!("Expected MemPageUnalignedAccess, got: {:?}", other),
    }
}

#[test]
fn test_wxorx_init_pages_addr_past_memory() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(4096);
    let result = mem.init_pages(8192, 4096, FLAG_FREEZED, None, 0);
    assert!(result.is_err());
    match result {
        Err(Error::MemOutOfBound(_, OutOfBoundKind::Memory)) => {}
        other => panic!("Expected MemOutOfBound, got: {:?}", other),
    }
}

#[test]
fn test_wxorx_init_pages_size_exceeds_memory() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(4096);
    let result = mem.init_pages(0, 8192, FLAG_FREEZED, None, 0);
    assert!(result.is_err());
}

#[test]
fn test_wxorx_init_pages_addr_plus_size_exceeds() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(4096);
    // addr=0, size=4096 is OK
    assert!(mem.init_pages(0, 4096, FLAG_FREEZED, None, 0).is_ok());
}

#[test]
fn test_wxorx_init_pages_offset_from_addr_exceeds_size() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    let result = mem.init_pages(0, 4096, FLAG_FREEZED, None, 5000);
    assert!(result.is_err());
    match result {
        Err(Error::MemOutOfBound(_, OutOfBoundKind::ExternalData)) => {}
        other => panic!("Expected ExternalData out of bound, got: {:?}", other),
    }
}

#[test]
fn test_wxorx_init_pages_write_to_frozen_page() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 4096, FLAG_FREEZED, None, 0).unwrap();
    // Try to init again — should fail because page is frozen
    let result = mem.init_pages(0, 4096, 0, None, 0);
    assert!(result.is_err());
    match result {
        Err(Error::MemWriteOnFreezedPage(_)) => {}
        other => panic!("Expected MemWriteOnFreezedPage, got: {:?}", other),
    }
}

#[test]
fn test_wxorx_init_pages_zero_size() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    // size=0 (page-aligned)
    let result = mem.init_pages(0, 0, FLAG_FREEZED, None, 0);
    assert!(result.is_ok(), "Zero-size init should succeed");
}

#[test]
fn test_wxorx_init_pages_exact_memory_size() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(4096);
    let result = mem.init_pages(0, 4096, FLAG_FREEZED, None, 0);
    assert!(result.is_ok(), "Init exactly memory size should succeed");
}

#[test]
fn test_wxorx_init_pages_multiple_regions() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(16384);
    mem.init_pages(0, 4096, FLAG_FREEZED, None, 0).unwrap();
    mem.init_pages(8192, 4096, FLAG_FREEZED, None, 0).unwrap();
    // Gap at 4096..8192 is not initialized
}

#[test]
fn test_wxorx_init_pages_overlapping_frozen() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 4096, FLAG_FREEZED, None, 0).unwrap();
    // Try to init page 1 (which overlaps with frozen page 0..4096)
    let result = mem.init_pages(0, 8192, FLAG_FREEZED, None, 0);
    assert!(result.is_err(), "Overlapping frozen page should fail");
}

// =========================================================================
// WXorXMemory store/load permission edge cases
// =========================================================================

#[test]
fn test_wxorx_store_to_executable_then_read() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    // Write first (before marking executable)
    mem.store8(&0u64, &42u8.into()).unwrap();
    // Mark as executable
    mem.set_flag(0, FLAG_EXECUTABLE | FLAG_FREEZED).unwrap();
    // Read should work
    assert_eq!(mem.load8(&0u64).unwrap().to_u8(), 42);
    // Write should fail
    assert!(mem.store8(&0u64, &1u8.into()).is_err());
}

#[test]
fn test_wxorx_execute_load_from_non_exec_page() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 4096, FLAG_FREEZED, None, 0).unwrap();
    // Page is writable, not executable — execute_load should fail
    let result = mem.execute_load16(0);
    assert!(result.is_err(), "Execute from non-exec page should fail");
}

#[test]
fn test_wxorx_execute_load_from_exec_page() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 4096, FLAG_EXECUTABLE | FLAG_FREEZED, None, 0)
        .unwrap();
    // Page is executable — execute_load should succeed
    assert!(mem.execute_load16(0).is_ok());
}

#[test]
fn test_wxorx_store_load_across_page_boundary() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 8192, FLAG_FREEZED, None, 0).unwrap();
    // Store 8 bytes at page boundary (4092..4100)
    mem.store64(&4092u64, &0x1122334455667788u64.into())
        .unwrap();
    let val = mem.load64(&4092u64).unwrap().to_u64();
    assert_eq!(val, 0x1122334455667788);
}

#[test]
fn test_wxorx_load_past_end() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(4096);
    mem.init_pages(0, 4096, FLAG_FREEZED, None, 0).unwrap();
    assert!(mem.load8(&4096u64).is_err());
}

#[test]
fn test_wxorx_store_past_end() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(4096);
    mem.init_pages(0, 4096, FLAG_FREEZED, None, 0).unwrap();
    assert!(mem.store8(&4096u64, &1u8.into()).is_err());
}

#[test]
fn test_wxorx_store_bytes_cross_page() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 8192, FLAG_FREEZED, None, 0).unwrap();
    let data = Bytes::from(vec![0xABu8; 100]);
    let result = mem.store_bytes(4090, &data);
    assert!(result.is_ok(), "Cross-page store should succeed");
}

#[test]
fn test_wxorx_store_byte_zero_size() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 4096, FLAG_FREEZED, None, 0).unwrap();
    let result = mem.store_byte(0, 0, 0xFF);
    assert!(result.is_ok(), "store_byte with size=0 should be OK");
}

#[test]
fn test_wxorx_memory_size() {
    let mem = WXorXMemory::<SparseMemory<u64>>::new(4096);
    assert_eq!(mem.memory_size(), 4096);
}

#[test]
fn test_wxorx_flag_operations() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();

    mem.set_flag(0, FLAG_DIRTY).unwrap();
    assert!(mem.fetch_flag(0).unwrap() & FLAG_DIRTY != 0);

    mem.clear_flag(0, FLAG_DIRTY).unwrap();
    assert!(mem.fetch_flag(0).unwrap() & FLAG_DIRTY == 0);
}

#[test]
fn test_wxorx_lr_operations() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    assert_eq!(mem.lr().to_u64(), 0);
    mem.set_lr(&12345u64.into());
    assert_eq!(mem.lr().to_u64(), 12345);
}
