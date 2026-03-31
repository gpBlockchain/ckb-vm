use ckb_vm::memory::{
    check_no_overflow, get_page_indices, load_c_string_byte_by_byte, sparse::SparseMemory,
};
use ckb_vm::Memory;
use ckb_vm::Register;
use ckb_vm_definitions::RISCV_PAGESIZE;

#[test]
fn test_check_no_overflow_exact_fit() {
    assert!(check_no_overflow(0, 100, 100).is_ok());
}

#[test]
fn test_check_no_overflow_one_past() {
    assert!(check_no_overflow(0, 101, 100).is_err());
}

#[test]
fn test_check_no_overflow_addr_at_end() {
    assert!(check_no_overflow(100, 1, 100).is_err());
}

#[test]
fn test_check_no_overflow_addr_past_end() {
    assert!(check_no_overflow(101, 1, 100).is_err());
}

#[test]
fn test_check_no_overflow_zero_size() {
    assert!(check_no_overflow(50, 0, 100).is_ok());
}

#[test]
fn test_check_no_overflow_size_one() {
    assert!(check_no_overflow(99, 1, 100).is_ok());
    assert!(check_no_overflow(100, 1, 100).is_err());
}

#[test]
fn test_check_no_overflow_u64_overflow() {
    assert!(check_no_overflow(u64::MAX, 1, u64::MAX).is_err());
}

#[test]
fn test_check_no_overflow_large_addr_small_memory() {
    assert!(check_no_overflow(1000, 1, 100).is_err());
}

#[test]
fn test_check_no_overflow_zero_memory() {
    assert!(check_no_overflow(0, 1, 0).is_err());
}

#[test]
fn test_check_no_overflow_max_values() {
    assert!(check_no_overflow(u64::MAX - 5, 10, u64::MAX).is_err());
}

#[test]
fn test_get_page_indices_single_page() {
    let (start, end) = get_page_indices(0, RISCV_PAGESIZE as u64);
    assert_eq!(start, 0);
    assert_eq!(end, 0);
}

#[test]
fn test_get_page_indices_two_pages() {
    let (start, end) = get_page_indices(0, 2 * RISCV_PAGESIZE as u64);
    assert_eq!(start, 0);
    assert_eq!(end, 1);
}

#[test]
fn test_get_page_indices_straddling() {
    let (start, end) = get_page_indices(RISCV_PAGESIZE as u64 - 4, 8);
    assert_eq!(start, 0);
    assert_eq!(end, 1);
}

#[test]
fn test_get_page_indices_at_boundary() {
    let (start, end) = get_page_indices(RISCV_PAGESIZE as u64, RISCV_PAGESIZE as u64);
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
fn test_get_page_indices_last_byte() {
    let (start, end) = get_page_indices(RISCV_PAGESIZE as u64 - 1, 1);
    assert_eq!(start, 0);
    assert_eq!(end, 0);
}

#[test]
fn test_get_page_indices_spanning_three_pages() {
    let (start, end) = get_page_indices(1, 3 * RISCV_PAGESIZE as u64 - 1);
    assert_eq!(start, 0);
    assert_eq!(end, 2);
}

#[test]
fn test_get_page_indices_spanning_four_pages() {
    let (start, end) = get_page_indices(RISCV_PAGESIZE as u64 - 1, 2 * RISCV_PAGESIZE as u64 + 2);
    assert_eq!(start, 0);
    assert_eq!(end, 3);
}

#[test]
fn test_get_page_indices_wraparound_should_not_reverse_range() {
    let (start, end) = get_page_indices(u64::MAX, 2);
    assert!(
        start <= end,
        "page index range should not reverse on address overflow"
    );
}

// --- Iteration 36: load_c_string_byte_by_byte edge cases ---

#[test]
fn test_load_c_string_empty_string() {
    let mut mem = SparseMemory::<u64>::new(4096);
    mem.store8(&0u64, &0u8.into()).unwrap(); // null terminator at addr 0
    let result = load_c_string_byte_by_byte(&mut mem, &0u64).unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
fn test_load_c_string_single_char() {
    let mut mem = SparseMemory::<u64>::new(4096);
    mem.store8(&0u64, &b'A'.into()).unwrap();
    mem.store8(&1u64, &0u8.into()).unwrap();
    let result = load_c_string_byte_by_byte(&mut mem, &0u64).unwrap();
    assert_eq!(&result[..], b"A");
}

#[test]
fn test_load_c_string_long_string() {
    let mut mem = SparseMemory::<u64>::new(8192);
    let test_str = b"Hello, CKB-VM World!";
    for (i, &b) in test_str.iter().enumerate() {
        mem.store8(&(i as u64), &b.into()).unwrap();
    }
    mem.store8(&(test_str.len() as u64), &0u8.into()).unwrap();
    let result = load_c_string_byte_by_byte(&mut mem, &0u64).unwrap();
    assert_eq!(&result[..], test_str);
}

#[test]
fn test_load_c_string_at_page_boundary() {
    let mut mem = SparseMemory::<u64>::new(8192);
    // String starting at last byte of page 0
    mem.store8(&4095u64, &b'Z'.into()).unwrap();
    mem.store8(&4096u64, &b'X'.into()).unwrap();
    mem.store8(&4097u64, &0u8.into()).unwrap();
    let result = load_c_string_byte_by_byte(&mut mem, &4095u64).unwrap();
    assert_eq!(&result[..], b"ZX");
}

#[test]
fn test_load_c_string_no_null_terminator() {
    let mut mem = SparseMemory::<u64>::new(4096);
    // Fill memory with non-zero values - no null terminator
    for i in 0u64..128 {
        mem.store8(&i, &0xFFu8.into()).unwrap();
    }
    // This should eventually error when it hits unmapped memory
    let result = load_c_string_byte_by_byte(&mut mem, &0u64);
    assert!(result.is_err());
}

#[test]
fn test_load_c_string_at_high_address() {
    let mut mem = SparseMemory::<u64>::new(8192);
    mem.store8(&8000u64, &b'A'.into()).unwrap();
    mem.store8(&8001u64, &0u8.into()).unwrap();
    let result = load_c_string_byte_by_byte(&mut mem, &8000u64).unwrap();
    assert_eq!(&result[..], b"A");
}
