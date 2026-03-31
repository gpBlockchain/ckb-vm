use bytes::Bytes;
use ckb_vm::bits::{rounddown, roundup};
use ckb_vm::error::{Error, OutOfBoundKind};
use ckb_vm::machine::{DefaultCoreMachine, VERSION2};
use ckb_vm::memory::{
    check_no_overflow, flat::FlatMemory, get_page_indices, sparse::SparseMemory,
    wxorx::WXorXMemory, FLAG_DIRTY, FLAG_EXECUTABLE, FLAG_FREEZED, FLAG_WRITABLE,
};
use ckb_vm::{Memory, Register, SupportMachine, ISA_IMC};

// =========================================================================
// bits::roundup / rounddown edge cases
// =========================================================================

#[test]
fn test_roundup_zero() {
    // roundup(0, any_power_of_2) should be 0
    assert_eq!(roundup(0, 1), 0);
    assert_eq!(roundup(0, 4096), 0);
}

#[test]
fn test_rounddown_zero() {
    // rounddown(0, any_power_of_2) should be 0
    assert_eq!(rounddown(0, 1), 0);
    assert_eq!(rounddown(0, 4096), 0);
}

#[test]
fn test_roundup_already_aligned() {
    // Already aligned values should be unchanged
    assert_eq!(roundup(4096, 4096), 4096);
    assert_eq!(roundup(8192, 4096), 8192);
    assert_eq!(roundup(1, 1), 1);
}

#[test]
fn test_rounddown_already_aligned() {
    assert_eq!(rounddown(4096, 4096), 4096);
    assert_eq!(rounddown(8192, 4096), 8192);
    assert_eq!(rounddown(1, 1), 1);
}

#[test]
fn test_roundup_unaligned() {
    assert_eq!(roundup(1, 4096), 4096);
    assert_eq!(roundup(4097, 4096), 8192);
    assert_eq!(roundup(2, 4), 4);
    assert_eq!(roundup(3, 4), 4);
}

#[test]
fn test_rounddown_unaligned() {
    assert_eq!(rounddown(4095, 4096), 0);
    assert_eq!(rounddown(8191, 4096), 4096);
    assert_eq!(rounddown(3, 4), 0);
    assert_eq!(rounddown(5, 4), 4);
}

#[test]
fn test_roundup_rounddown_roundtrip() {
    // For any x >= round, rounddown then roundup should give the next aligned boundary
    for x in [1u64, 42, 100, 4095, 4096, 4097, 10000, u64::MAX - 4096] {
        let round = 4096u64;
        let down = rounddown(x, round);
        let up = roundup(x, round);
        assert!(down <= x, "rounddown should be <= x");
        assert!(up >= x, "roundup should be >= x");
        if x % round == 0 {
            assert_eq!(down, x, "rounddown of aligned value");
            assert_eq!(up, x, "roundup of aligned value");
        }
    }
}

#[test]
fn test_roundup_with_1() {
    // roundup(x, 1) should always be x
    for x in [0u64, 1, 100, u64::MAX] {
        assert_eq!(roundup(x, 1), x);
    }
}

#[test]
fn test_rounddown_with_1() {
    // rounddown(x, 1) should always be x
    for x in [0u64, 1, 100, u64::MAX] {
        assert_eq!(rounddown(x, 1), x);
    }
}

#[test]
fn test_roundup_large_values() {
    // Near u64::MAX
    assert_eq!(roundup(u64::MAX - 1, 4096), 0); // wraps around
    assert_eq!(roundup(u64::MAX, 4096), 0); // wraps around
}

#[test]
fn test_rounddown_large_values() {
    assert_eq!(rounddown(u64::MAX, 4096), u64::MAX & !4095);
    assert_eq!(rounddown(u64::MAX - 1, 4096), u64::MAX & !4095);
}

// =========================================================================
// memory::check_no_overflow edge cases
// =========================================================================

#[test]
fn test_check_no_overflow_zero_size() {
    // size=0: addr must still be < memory_size
    assert!(check_no_overflow(0, 0, 100).is_ok());
    assert!(check_no_overflow(50, 0, 100).is_ok());
    // addr >= memory_size is still an error
    assert!(check_no_overflow(100, 0, 100).is_err());
    assert!(check_no_overflow(200, 0, 100).is_err());
}

#[test]
fn test_check_no_overflow_exact_fit() {
    // addr + size == memory_size should be OK
    assert!(check_no_overflow(0, 100, 100).is_ok());
    assert!(check_no_overflow(50, 50, 100).is_ok());
    assert!(check_no_overflow(99, 1, 100).is_ok());
}

#[test]
fn test_check_no_overflow_one_past() {
    // addr + size == memory_size + 1 should fail
    let result = check_no_overflow(0, 101, 100);
    assert!(result.is_err());
    match result {
        Err(Error::MemOutOfBound(_, OutOfBoundKind::Memory)) => {}
        other => panic!("Expected MemOutOfBound, got: {:?}", other),
    }
}

#[test]
fn test_check_no_overflow_addr_at_memory_size() {
    // addr == memory_size should fail (even with size=0)
    let result = check_no_overflow(100, 0, 100);
    assert!(result.is_err());
}

#[test]
fn test_check_no_overflow_addr_past_memory_size() {
    let result = check_no_overflow(101, 0, 100);
    assert!(result.is_err());
}

#[test]
fn test_check_no_overflow_add_overflow() {
    // addr + size overflows u64
    let result = check_no_overflow(u64::MAX, 1, u64::MAX);
    assert!(result.is_err());
    match result {
        Err(Error::MemOutOfBound(_, OutOfBoundKind::Memory)) => {}
        other => panic!("Expected MemOutOfBound, got: {:?}", other),
    }
}

#[test]
fn test_check_no_overflow_both_max() {
    let result = check_no_overflow(u64::MAX, u64::MAX, u64::MAX);
    assert!(result.is_err());
}

#[test]
fn test_check_no_overflow_size_one() {
    assert!(check_no_overflow(0, 1, 100).is_ok());
    assert!(check_no_overflow(99, 1, 100).is_ok());
    assert!(check_no_overflow(100, 1, 100).is_err());
}

#[test]
fn test_check_no_overflow_max_memory() {
    // Full range of u64 memory
    assert!(check_no_overflow(0, u64::MAX, u64::MAX).is_ok());
    assert!(check_no_overflow(u64::MAX - 1, 1, u64::MAX).is_ok());
}

// =========================================================================
// memory::get_page_indices edge cases
// =========================================================================

#[test]
fn test_get_page_indices_single_page() {
    let (start, end) = get_page_indices(0, 4096);
    assert_eq!(start, 0);
    assert_eq!(end, 0);
}

#[test]
fn test_get_page_indices_two_pages() {
    let (start, end) = get_page_indices(0, 4097);
    assert_eq!(start, 0);
    assert_eq!(end, 1);
}

#[test]
fn test_get_page_indices_at_boundary() {
    // Address at exactly a page boundary
    let (start, end) = get_page_indices(4096, 4096);
    assert_eq!(start, 1);
    assert_eq!(end, 1);
}

#[test]
fn test_get_page_indices_one_byte() {
    let (start, end) = get_page_indices(0, 1);
    assert_eq!(start, 0);
    assert_eq!(end, 0);
}

#[test]
fn test_get_page_indices_crossing_boundary() {
    // One byte before boundary, one after
    let (start, end) = get_page_indices(4095, 2);
    assert_eq!(start, 0);
    assert_eq!(end, 1);
}

#[test]
fn test_get_page_indices_large_addr() {
    let (start, end) = get_page_indices(u64::MAX - 1, 1);
    let expected_page = (u64::MAX - 1) >> 12;
    assert_eq!(start, expected_page);
    assert_eq!(end, expected_page);
}

// =========================================================================
// FlatMemory boundary tests
// =========================================================================

#[test]
fn test_flat_memory_load_at_boundary() {
    let mut mem = FlatMemory::<u64>::new(8192);
    mem.init_pages(0, 8192, 0, None, 0).unwrap();
    mem.store8(&0u64, &1u8.into()).unwrap();
    mem.store8(&4095u64, &2u8.into()).unwrap();
    mem.store8(&4096u64, &3u8.into()).unwrap();
    mem.store8(&8191u64, &4u8.into()).unwrap();

    assert_eq!(mem.load8(&0u64).unwrap().to_u8(), 1);
    assert_eq!(mem.load8(&4095u64).unwrap().to_u8(), 2);
    assert_eq!(mem.load8(&4096u64).unwrap().to_u8(), 3);
    assert_eq!(mem.load8(&8191u64).unwrap().to_u8(), 4);
}

#[test]
fn test_flat_memory_load_past_end() {
    let mut mem = FlatMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    let result = mem.load8(&4096u64);
    assert!(result.is_err(), "Load past end should fail");
}

#[test]
fn test_flat_memory_store_past_end() {
    let mut mem = FlatMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    let result = mem.store8(&4096u64, &1u8.into());
    assert!(result.is_err(), "Store past end should fail");
}

#[test]
fn test_flat_memory_load64_near_end() {
    let mut mem = FlatMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    // 8 bytes at addr 4088 should fit
    let result = mem.load64(&4088u64);
    assert!(result.is_ok(), "Load64 at 4088 in 4096-byte mem should fit");
    // 8 bytes at addr 4089 should NOT fit
    let result = mem.load64(&4089u64);
    assert!(
        result.is_err(),
        "Load64 at 4089 in 4096-byte mem should fail"
    );
}

#[test]
fn test_flat_memory_store64_near_end() {
    let mut mem = FlatMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    // 8 bytes at addr 4088 should fit
    let result = mem.store64(&4088u64, &0u64.into());
    assert!(
        result.is_ok(),
        "Store64 at 4088 in 4096-byte mem should fit"
    );
    // 8 bytes at addr 4089 should NOT fit
    let result = mem.store64(&4089u64, &0u64.into());
    assert!(
        result.is_err(),
        "Store64 at 4089 in 4096-byte mem should fail"
    );
}

#[test]
fn test_flat_memory_load16_near_end() {
    let mut mem = FlatMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    assert!(mem.load16(&4094u64).is_ok());
    assert!(mem.load16(&4095u64).is_err());
}

#[test]
fn test_flat_memory_store16_near_end() {
    let mut mem = FlatMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    assert!(mem.store16(&4094u64, &0u16.into()).is_ok());
    assert!(mem.store16(&4095u64, &0u16.into()).is_err());
}

#[test]
fn test_flat_memory_zero_size() {
    let mem = FlatMemory::<u64>::new(0);
    assert_eq!(mem.memory_size(), 0);
}

#[test]
fn test_flat_memory_store_byte_zero_size() {
    let mut mem = FlatMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    // store_byte with size=0
    let result = mem.store_byte(0, 0, 0xAA);
    assert!(result.is_ok(), "store_byte with size=0 should be OK");
}

// =========================================================================
// SparseMemory boundary tests
// =========================================================================

#[test]
fn test_sparse_memory_load_past_end() {
    let mut mem = SparseMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    let result = mem.load8(&4096u64);
    assert!(result.is_err(), "Load past end should fail");
}

#[test]
fn test_sparse_memory_store_past_end() {
    let mut mem = SparseMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    let result = mem.store8(&4096u64, &1u8.into());
    assert!(result.is_err(), "Store past end should fail");
}

#[test]
fn test_sparse_memory_store_past_end_should_not_partially_write() {
    let mut mem = SparseMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();

    // Write two bytes starting from last valid byte. This should fail.
    let result = mem.store_bytes(4095, &[0xAA, 0xBB]);
    assert!(result.is_err(), "Cross-end store should fail");

    // Failing store should not modify in-range bytes.
    assert_eq!(mem.load8(&4095u64).unwrap().to_u8(), 0);
}

#[test]
fn test_sparse_memory_store_byte_past_end_should_not_partially_write() {
    let mut mem = SparseMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();

    // memset-like write crossing end should fail.
    let result = mem.store_byte(4095, 2, 0xCC);
    assert!(result.is_err(), "Cross-end store_byte should fail");

    // Failing store should not modify in-range bytes.
    assert_eq!(mem.load8(&4095u64).unwrap().to_u8(), 0);
}

#[test]
fn test_sparse_memory_store64_cross_end_should_not_partially_write() {
    let mut mem = SparseMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();

    // 8-byte store from 4092 crosses memory end and should fail atomically.
    let result = mem.store64(&4092u64, &0x1122334455667788u64.into());
    assert!(result.is_err(), "Cross-end store64 should fail");

    // Bytes in valid region should remain unchanged.
    for i in 4092u64..4096u64 {
        assert_eq!(mem.load8(&i).unwrap().to_u8(), 0);
    }
}

#[test]
fn test_sparse_memory_store16_cross_end_should_not_partially_write() {
    let mut mem = SparseMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();

    let result = mem.store16(&4095u64, &0xABCDu16.into());
    assert!(result.is_err(), "Cross-end store16 should fail");

    assert_eq!(mem.load8(&4095u64).unwrap().to_u8(), 0);
}

#[test]
fn test_sparse_memory_load64_near_end() {
    let mut mem = SparseMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    assert!(mem.load64(&4088u64).is_ok());
    assert!(mem.load64(&4089u64).is_err());
}

#[test]
fn test_sparse_memory_store64_near_end() {
    let mut mem = SparseMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    assert!(mem.store64(&4088u64, &0u64.into()).is_ok());
    assert!(mem.store64(&4089u64, &0u64.into()).is_err());
}

#[test]
fn test_sparse_memory_store_bytes_across_pages() {
    let mut mem = SparseMemory::<u64>::new(8192);
    mem.init_pages(0, 8192, 0, None, 0).unwrap();
    // Write 100 bytes starting at page boundary
    let data = vec![0xABu8; 100];
    let result = mem.store_bytes(4096 - 50, &Bytes::from(data));
    assert!(result.is_ok(), "Cross-page store should succeed");
}

#[test]
fn test_sparse_memory_load_bytes_near_end() {
    let mut mem = SparseMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    // Load 10 bytes starting at 4090 — should fail (6 bytes past end)
    let result = mem.load_bytes(4090, 10);
    assert!(result.is_err(), "Load past end should fail");
}

#[test]
fn test_sparse_memory_load_bytes_at_end() {
    let mut mem = SparseMemory::<u64>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    // Load 0 bytes at 4096 — returns empty (size=0 is a no-op)
    let result = mem.load_bytes(4096, 0);
    assert!(result.is_ok(), "Load 0 bytes should be OK");
    assert!(result.unwrap().is_empty());
    // Load 1 byte at 4096 — should fail
    let result = mem.load_bytes(4096, 1);
    assert!(result.is_err(), "Load 1 byte past end should fail");
}

// =========================================================================
// WXorXMemory permission tests
// =========================================================================

#[test]
fn test_wxorx_memory_store_to_executable_page() {
    use ckb_vm::memory::{FLAG_EXECUTABLE, FLAG_FREEZED};

    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 8192, 0, None, 0).unwrap();

    // Mark page 0 as executable
    mem.set_flag(0, FLAG_EXECUTABLE | FLAG_FREEZED).unwrap();

    // Try to write to executable page — should fail
    let result = mem.store8(&0u64, &1u8.into());
    assert!(result.is_err(), "Write to executable page should fail");
}

#[test]
fn test_wxorx_memory_load_from_executable_page() {
    use ckb_vm::memory::{FLAG_EXECUTABLE, FLAG_FREEZED};

    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 8192, 0, None, 0).unwrap();

    // First write to non-exec page, then mark as executable
    mem.store8(&0u64, &42u8.into()).unwrap();
    mem.set_flag(0, FLAG_EXECUTABLE | FLAG_FREEZED).unwrap();

    // Read from executable page — should be OK
    let result = mem.load8(&0u64);
    assert!(result.is_ok(), "Read from executable page should succeed");
    assert_eq!(result.unwrap().to_u8(), 42);
}

#[test]
fn test_wxorx_memory_store_to_writable_page() {
    use ckb_vm::memory::FLAG_FREEZED;

    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 8192, 0, None, 0).unwrap();

    // Mark page 0 as writable (frozen, not executable)
    mem.set_flag(0, FLAG_FREEZED).unwrap();

    // Write to writable page — should succeed
    let result = mem.store8(&0u64, &1u8.into());
    assert!(result.is_ok(), "Write to writable page should succeed");
}

#[test]
fn test_wxorx_memory_load64_at_page_boundary() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 8192, 0, None, 0).unwrap();

    // Store at page boundary, then load spanning boundary
    mem.store64(&4092u64, &0x1122334455667788u64.into())
        .unwrap();
    let result = mem.load64(&4092u64);
    assert!(result.is_ok(), "Load64 at page boundary should succeed");
}

#[test]
fn test_wxorx_memory_store_past_end() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    let result = mem.store8(&4096u64, &1u8.into());
    assert!(result.is_err(), "Store past end should fail");
}

// =========================================================================
// Memory size and init edge cases
// =========================================================================

#[test]
fn test_flat_memory_minimal_size() {
    let mut mem = FlatMemory::<u64>::new(4096); // must be page-aligned
    assert_eq!(mem.memory_size(), 4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    mem.store8(&0u64, &0xFFu8.into()).unwrap();
    assert_eq!(mem.load8(&0u64).unwrap().to_u8(), 0xFF);
}

#[test]
fn test_sparse_memory_minimal_size() {
    let mut mem = SparseMemory::<u64>::new(4096); // must be page-aligned
    assert_eq!(mem.memory_size(), 4096);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    mem.store8(&0u64, &0xFFu8.into()).unwrap();
    assert_eq!(mem.load8(&0u64).unwrap().to_u8(), 0xFF);
}

#[test]
fn test_flat_memory_init_multiple_regions() {
    let mut mem = FlatMemory::<u64>::new(16384);
    mem.init_pages(0, 4096, 0, None, 0).unwrap();
    mem.init_pages(8192, 4096, 0, None, 0).unwrap();
    // Gap at 4096..8192 is not initialized
    mem.store8(&0u64, &1u8.into()).unwrap();
    mem.store8(&8192u64, &2u8.into()).unwrap();
    assert_eq!(mem.load8(&0u64).unwrap().to_u8(), 1);
    assert_eq!(mem.load8(&8192u64).unwrap().to_u8(), 2);
}

#[test]
fn test_default_core_machine_memory_size() {
    let machine = DefaultCoreMachine::<u64, FlatMemory<u64>>::new(ISA_IMC, VERSION2, 1000);
    assert_eq!(machine.cycles(), 0);
    assert_eq!(machine.max_cycles(), 1000);
}

// --- Iteration 33: boundary tests for memory operations ---

#[test]
fn test_wxorx_store_at_exact_page_boundary() {
    // Store 1 byte at the very last byte of page 0
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 8192, FLAG_FREEZED, None, 0).unwrap();
    mem.store8(&4095u64, &0xAAu8.into()).unwrap();
    assert_eq!(mem.load8(&4095u64).unwrap().to_u8(), 0xAA);
}

#[test]
fn test_wxorx_store32_at_page_boundary_straddling() {
    // Store 4 bytes starting at 4094 (spans page boundary 4096)
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 8192, FLAG_FREEZED, None, 0).unwrap();
    mem.store32(&4094u64, &0xDEADBEEFu32.into()).unwrap();
    let val = mem.load32(&4094u64).unwrap().to_u32();
    assert_eq!(val, 0xDEADBEEF);
}

#[test]
fn test_wxorx_store8_at_last_byte_of_memory() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(4096);
    mem.init_pages(0, 4096, FLAG_FREEZED, None, 0).unwrap();
    mem.store8(&4095u64, &0xBBu8.into()).unwrap();
    assert_eq!(mem.load8(&4095u64).unwrap().to_u8(), 0xBB);
}

#[test]
fn test_wxorx_store16_at_last_two_bytes_fails() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(4096);
    mem.init_pages(0, 4096, FLAG_FREEZED, None, 0).unwrap();
    // addr=4095, size=2 → 4095+2 = 4097 > 4096, should fail
    let result = mem.store16(&4095u64, &0xCCDDu16.into());
    assert!(result.is_err());
}

#[test]
fn test_wxorx_permission_transition_writable_to_executable() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    // Page 0: writable, page 1: executable
    mem.init_pages(0, 4096, FLAG_WRITABLE | FLAG_FREEZED, None, 0)
        .unwrap();
    mem.init_pages(4096, 4096, FLAG_EXECUTABLE | FLAG_FREEZED, None, 0)
        .unwrap();

    // Write to page 0 should work
    mem.store8(&0u64, &1u8.into()).unwrap();
    // Execute from page 1 should work
    assert!(mem.execute_load16(4096).is_ok());
    // Write to page 1 should fail
    assert!(mem.store8(&4096u64, &1u8.into()).is_err());
}

#[test]
fn test_flat_store_at_last_valid_byte() {
    let mut mem = FlatMemory::<u64>::new(4096);
    mem.store8(&4095u64, &0xEEu8.into()).unwrap();
    assert_eq!(mem.load8(&4095u64).unwrap().to_u8(), 0xEE);
}

#[test]
fn test_flat_store16_at_last_byte_fails() {
    let mut mem = FlatMemory::<u64>::new(4096);
    let result = mem.store16(&4095u64, &0x1234u16.into());
    assert!(result.is_err());
}

#[test]
fn test_flat_store64_at_exact_middle() {
    let mut mem = FlatMemory::<u64>::new(4096);
    let addr = 4096u64 - 8;
    mem.store64(&addr, &0x1122334455667788u64.into()).unwrap();
    assert_eq!(mem.load64(&addr).unwrap().to_u64(), 0x1122334455667788);
}

#[test]
fn test_sparse_store_bytes_zero_length() {
    let mut mem = SparseMemory::<u64>::new(4096);
    let result = mem.store_bytes(0, &[]);
    assert!(result.is_ok(), "Zero-length store should succeed");
}

#[test]
fn test_sparse_store_bytes_at_zero_addr() {
    let mut mem = SparseMemory::<u64>::new(4096);
    mem.store_bytes(0, &[0x42; 100]).unwrap();
    assert_eq!(mem.load8(&0u64).unwrap().to_u8(), 0x42);
    assert_eq!(mem.load8(&99u64).unwrap().to_u8(), 0x42);
}

#[test]
fn test_sparse_store_byte_zero_count() {
    let mut mem = SparseMemory::<u64>::new(4096);
    let result = mem.store_byte(0, 0, 0xFF);
    assert!(result.is_ok(), "store_byte with count=0 should succeed");
}

#[test]
fn test_get_page_indices_single_byte_at_page_start() {
    let (start, end) = get_page_indices(0, 1);
    assert_eq!(start, 0);
    assert_eq!(end, 0);
}

#[test]
fn test_get_page_indices_single_byte_at_page_end() {
    let (start, end) = get_page_indices(4095, 1);
    assert_eq!(start, 0);
    assert_eq!(end, 0);
}

#[test]
fn test_get_page_indices_exactly_two_pages() {
    // addr=4095, size=2 → pages 0 and 1
    let (start, end) = get_page_indices(4095, 2);
    assert_eq!(start, 0);
    assert_eq!(end, 1);
}

#[test]
fn test_get_page_indices_full_three_pages() {
    // addr=0, size=12288 → pages 0, 1, 2
    let (start, end) = get_page_indices(0, 12288);
    assert_eq!(start, 0);
    assert_eq!(end, 2);
}

#[test]
fn test_check_no_overflow_at_exact_limit() {
    // addr=4088, size=8, memory_size=4096 → should pass
    let result = check_no_overflow(4088, 8, 4096);
    assert!(result.is_ok());
}

#[test]
fn test_check_no_overflow_one_past_limit() {
    // addr=4089, size=8, memory_size=4096 → should fail
    let result = check_no_overflow(4089, 8, 4096);
    assert!(result.is_err());
}

#[test]
fn test_check_no_overflow_addr_at_memory_size() {
    // addr=4096, size=1, memory_size=4096 → should fail (addr >= memory_size)
    let result = check_no_overflow(4096, 1, 4096);
    assert!(result.is_err());
}

#[test]
fn test_wxorx_cross_page_store_with_exec_on_second_page() {
    // First page writable, second page executable
    // Store straddling boundary should fail because second page is not writable
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 4096, FLAG_WRITABLE | FLAG_FREEZED, None, 0)
        .unwrap();
    mem.init_pages(4096, 4096, FLAG_EXECUTABLE | FLAG_FREEZED, None, 0)
        .unwrap();

    // store16 at 4095 straddles to page 1 which is exec-only → should fail
    let result = mem.store16(&4095u64, &0xBEEFu16.into());
    assert!(result.is_err(), "Cross-page write to exec page should fail");
}

#[test]
fn test_flat_memory_roundtrip_small_values() {
    let mut mem = FlatMemory::<u64>::new(4096);
    for i in 0u64..256 {
        mem.store8(&i, &(i as u8).into()).unwrap();
    }
    for i in 0u64..256 {
        assert_eq!(mem.load8(&i).unwrap().to_u8(), i as u8);
    }
}

#[test]
fn test_sparse_memory_roundtrip_across_pages() {
    let mut mem = SparseMemory::<u64>::new(8192);
    // Write across two pages
    mem.store64(&4090u64, &0xAAAABBBBCCCCDDDDu64.into())
        .unwrap();
    // Read back
    let val = mem.load64(&4090u64).unwrap().to_u64();
    assert_eq!(val, 0xAAAABBBBCCCCDDDD);
}

#[test]
fn test_wxorx_store_byte_at_page_start_after_init() {
    let mut mem = WXorXMemory::<SparseMemory<u64>>::new(8192);
    mem.init_pages(0, 8192, FLAG_FREEZED, None, 0).unwrap();
    mem.store_byte(0, 1, 0x11).unwrap();
    mem.store_byte(4096, 1, 0x22).unwrap();
    assert_eq!(mem.load8(&0u64).unwrap().to_u8(), 0x11);
    assert_eq!(mem.load8(&4096u64).unwrap().to_u8(), 0x22);
}
