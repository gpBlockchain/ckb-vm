# Bug-Hunt Context

## Project Understanding

CKB-VM is Nervos CKB's RISC-V virtual machine implementation in Rust. It supports multiple execution modes (interpreter, ASM), multiple memory backends (flat, sparse, WXORX), ELF binary loading, snapshot/resume, and RISC-V extensions (M/A/B/V). The VM executes RISC-V 64-bit instructions with cycle counting for consensus.

## Test Coverage Gaps

1. **Instruction decoding** — decode_mop and decode_raw with complex MOP patterns
2. **Snapshot/resume** — snapshot2 dirty page coalescing, DataSource failures
3. ~~**Stack initialization**~~ — tested: empty args, empty Bytes, zero stack size, large args, error propagation
4. **WXorX memory permissions** — write-then-execute, flag edge cases
5. **ASM memory checks** — writable/executable/inited checks with boundary addresses
6. **Cycle limit edge cases** — exactly at limit, one below, overflow
7. **Decoder cache** — instruction cache eviction, collision handling
8. **Snapshot page tracking** — track_pages/untrack_pages boundary conditions

## Known Bugs

1. **initialize_stack integer underflow** — when stack_size=0 and version >= VERSION1 with empty args, `origin_sp - argc_size` panics (attempt to subtract with overflow). Should return `MemOutOfStack` error instead.
   - File: `src/machine/mod.rs:203`
   - Evidence: `tests/test_initialize_stack.rs` → `test_initialize_stack_zero_stack_size` (#[should_panic])
   - Category: null-input / boundary

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
| malformed-input | test-added | 1 | 31 | iteration 1 |
| edge-case | test-added | 3 | 68 | iteration 4 |
| boundary | test-added | 1 | 42 | iteration 3 |
| error-path | test-added | 1 | 34 | iteration 5 |
| null-input | bug-found | 1 | 15 | iteration 6 |
