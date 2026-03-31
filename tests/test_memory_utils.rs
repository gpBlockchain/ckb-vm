use ckb_vm::memory::{check_no_overflow, get_page_indices};
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
