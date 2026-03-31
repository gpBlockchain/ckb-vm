# Bug-Hunt Context

## Project Understanding

CKB-VM is Nervos CKB's RISC-V virtual machine implementation in Rust. It supports multiple execution modes (interpreter, ASM), multiple memory backends (flat, sparse, WXORX), ELF binary loading, snapshot/resume, and RISC-V extensions (M/A/B/V). The VM executes RISC-V 64-bit instructions with cycle counting for consensus.

## Test Coverage Gaps

1. **Instruction decoding** — decode_mop and decode_raw with complex MOP patterns
2. **Snapshot/resume** — snapshot2 dirty page coalescing, DataSource failures — partially tested with 16 state-corruption tests
3. **Stack initialization** — tested: empty args, empty Bytes, zero stack size, large args, error propagation
4. **WXorX memory permissions** — write-then-execute, flag edge cases — tested with 16 edge-case/boundary tests
5. **ASM memory checks** — writable/executable/inited checks with boundary addresses
6. **Cycle limit edge cases** — exactly at limit, one below, overflow — tested with 13 boundary tests
7. **Decoder cache** — instruction cache eviction, collision handling — tested with 4 MOP edge-case tests
8. **Snapshot page tracking** — track_pages/untrack_pages boundary conditions (new wraparound panic found)

## Known Bugs

1. **initialize_stack integer underflow** — when stack_size=0 and version >= VERSION1 with empty args, `origin_sp - argc_size` panics (attempt to subtract with overflow). Should return `MemOutOfStack` error instead.
   - File: `src/machine/mod.rs:203`
   - Evidence: `tests/test_initialize_stack.rs` → `test_initialize_stack_zero_stack_size` (#[should_panic])
   - Category: null-input / boundary
2. **decode_mop fusion failure** — MOP fusion rules (ADC, ADD3) fail to match in certain instruction sequences even when conditions are met. Decoder returns individual instructions instead of fused one.
   - File: `src/decoder.rs:139`
   - Evidence: `tests/test_decode_mop.rs` → `test_decode_mop_adc_partial_match`
   - Category: edge-case
3. **snapshot2::track_pages arithmetic underflow** — when `start = u64::MAX` and `length = 1`, `roundup(start, PAGE_SIZE)` wraps to 0 and `aligned_start - start` underflows, causing panic (`attempt to subtract with overflow`) instead of returning safely.
   - File: `src/snapshot2.rs:251`
   - Evidence: `tests/test_snapshot2_state_corruption.rs` → `test_snapshot2_track_pages_wraparound_start_panics`
   - Category: edge-case
4. **parse_elf unchecked wraparound in segment math** — crafted PT_LOAD with `p_vaddr = u64::MAX` and `p_memsz = u64::MAX` is accepted; `size` and `addr` computations use wrapping arithmetic and produce wrapped `LoadingAction` instead of rejecting malformed input.
   - File: `src/elf.rs:184-186`
   - Evidence: `tests/test_elf_parsing.rs` → `test_parse_elf_rejects_wrapping_segment_size`
   - Category: malformed-input
5. **snapshot2::track_pages offset overflow** — with `start = 1`, `length = 4096`, and `offset = u64::MAX`, the `offset += aligned_bytes` update overflows and panics (`attempt to add with overflow`).
   - File: `src/snapshot2.rs:255`
   - Evidence: `tests/test_snapshot2_state_corruption.rs` → `test_snapshot2_track_pages_offset_overflow_panics`
   - Category: edge-case
6. **parse_elf missing filesz/memsz invariant check** — malformed PT_LOAD with `p_filesz > p_memsz` is accepted and converted into a load action, instead of rejecting invalid ELF segment layout.
   - File: `src/elf.rs:185-203`
   - Evidence: `tests/test_elf_parsing.rs` → `test_parse_elf_rejects_filesz_larger_than_memsz`
   - Category: malformed-input
7. **get_page_indices wraparound produces reversed range** — for `addr = u64::MAX, size = 2`, wrapped end address yields `page_start > page_end`; callers using `start..=end` silently skip permission/dirty loops.
   - File: `src/memory/mod.rs:119-122`
   - Evidence: `tests/test_memory_utils.rs` → `test_get_page_indices_wraparound_should_not_reverse_range`
   - Category: boundary
8. **snapshot2::untrack_pages silently succeeds on wrapped range** — `start = u64::MAX, length = 2` returns `Ok(())` instead of error because `get_page_indices` returns reversed indices and loop is skipped.
   - File: `src/snapshot2.rs:278-282`
   - Evidence: `tests/test_snapshot2_state_corruption.rs` → `test_snapshot2_untrack_pages_wraparound_should_error`
   - Category: edge-case
9. **snapshot2::untrack_pages silently succeeds on large wrapped range** — `start = u64::MAX, length = 4096` also returns `Ok(())`, confirming the bug is not limited to tiny lengths.
   - File: `src/snapshot2.rs:278-282`
   - Evidence: `tests/test_snapshot2_state_corruption.rs` → `test_snapshot2_untrack_pages_wraparound_large_length_should_error`
   - Category: edge-case
10. **sparse::store_bytes is non-atomic on failing cross-end write** — writing from the last valid byte with length 2 mutates in-range byte, then fails on next page fetch.
   - File: `src/memory/sparse.rs:164-183`
   - Evidence: `tests/test_memory_boundary.rs` → `test_sparse_memory_store_past_end_should_not_partially_write`
   - Category: boundary
11. **sparse::store_byte is non-atomic on failing cross-end memset** — `store_byte(4095, 2, value)` mutates byte 4095 before failing at next page.
   - File: `src/memory/sparse.rs:186-203`
   - Evidence: `tests/test_memory_boundary.rs` → `test_sparse_memory_store_byte_past_end_should_not_partially_write`
   - Category: boundary
12. **sparse::store64 is non-atomic on failing cross-end write** — `store64(4092, value)` mutates bytes in `[4092, 4095]` then fails when stepping to non-existent next page.
   - File: `src/memory/sparse.rs:252-268` (via `store_bytes`)
   - Evidence: `tests/test_memory_boundary.rs` → `test_sparse_memory_store64_cross_end_should_not_partially_write`
   - Category: boundary
13. **sparse::store16 is non-atomic on failing cross-end write** — `store16(4095, value)` mutates byte 4095 before failing.
   - File: `src/memory/sparse.rs:232-236` (via `store_bytes`)
   - Evidence: `tests/test_memory_boundary.rs` → `test_sparse_memory_store16_cross_end_should_not_partially_write`
   - Category: boundary

## What Works

- Basic VM execution with both Rust and ASM backends
- RISC-V instruction execution across versions (v0, v1, v2)
- Memory load/store operations in common cases
- ELF loading for well-formed binaries
- Snapshot creation and resume
- MOP (macro-operation fusion) instructions
- A/B extensions basic functionality
- ELF parsing: convert_flags flag validation, parse_elf with invalid inputs
- Version 0 vs 1+ writable segment freeze behavior
- Memory boundary operations across flat/sparse/wxorx backends
- Instruction type constructors (Rtype/Itype/Stype/Utype/R4type/R5type)
- Error type Display formatting and trait implementations
- RNG deterministic behavior and cost model cycle counting
- Snapshot2 state: dirty page coalescing, register preservation, unaligned error detection
- InitializeStack: empty args, error propagation, SP alignment across versions
- WXorX memory: init_pages validation, permission checks, page boundary straddling
- Cycle management: add_cycles at limit, overflow, accumulation; check_no_overflow and get_page_indices

## Ideas Backlog — Tests to Write

1. **MOP decode patterns** — test each fusion rule with partial matches
2. **Snapshot page tracking** — track/untrack boundary pages, overlapping regions
3. **Stack init with edge cases** — empty args, huge args, null pointers
4. **WXorX permission violations** — write then execute attempts
5. **Cycle limit edge cases** — exactly at limit, one below, overflow
6. **Decoder cache collisions** — same cache slot, different PCs
7. **Memory page flag operations** — concurrent flag changes
8. **ASM machine step** — step with various instruction types

## Categories Tried

| Category | Type | Attempts | Kept | Last Tried |
|----------|------|----------|------|------------|
| malformed-input | test-added | 2 | 34 | iteration 14 |
| malformed-input | bug-found | 4 | 36 | iteration 21 |
| edge-case | bug-found | 9 | 94 | iteration 27 |
| boundary | bug-found | 7 | 82 | iteration 30 |
| boundary | test-added | 2 | 73 | iteration 10 |
| error-path | test-added | 1 | 34 | iteration 5 |
| null-input | bug-found | 1 | 15 | iteration 6 |
| state-corruption | test-added | 3 | 20 | iteration 16 |
