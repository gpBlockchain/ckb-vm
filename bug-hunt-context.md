# Bug-Hunt Context

## Project Understanding

CKB-VM is Nervos CKB's RISC-V virtual machine implementation in Rust. It supports multiple execution modes (interpreter, ASM), multiple memory backends (flat, sparse, WXORX), ELF binary loading, snapshot/resume, and RISC-V extensions (M/A/B/V). The VM executes RISC-V 64-bit instructions with cycle counting for consensus.

## Test Coverage Gaps

1. **Memory boundary conditions** — page boundary crossing loads/stores, zero-length operations, maximum address access
2. **Instruction decoding** — invalid opcodes, truncated instructions, malformed encoding fields
3. **Snapshot/resume** — corrupted snapshots, partial resume data, memory state consistency
4. **Stack initialization** — oversized arguments, empty args, null bytes in strings
5. **WXorX memory permissions** — write-then-execute violations, permission edge cases
6. **ASM memory checks** — writable/executable/inited checks with boundary addresses
7. **Bits utilities** — roundup/rounddown with non-power-of-2 inputs, zero, max values
8. **Cost model** — complete opcode coverage, unknown opcode handling
9. **Error propagation** — ensure all error paths are exercised and properly typed
10. **Decoder edge cases** — decode_mop and decode_raw with boundary instruction encodings

## Known Bugs

(None found yet — 203 tests passing, 0 failing)

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

## Ideas Backlog — Tests to Write

1. **Boundary tests for memory operations** — addresses at page boundaries, max u64, zero
2. **Invalid instruction encodings** — unknown opcodes, truncated instructions
3. **Bits utility edge cases** — zero inputs, non-power-of-2, overflow conditions
4. **Stack init with edge cases** — empty args, huge args, null pointers
5. **WXorX permission violations** — write then execute attempts
6. **Snapshot corruption** — modified snapshot data, missing fields
7. **Cycle limit edge cases** — exactly at limit, one below, overflow
8. **Memory page flag operations** — boundary pages, concurrent flag changes
9. **Error type conversions** — all error variants from external crates
10. **Decoder boundary encodings** — decode_mop with edge case instruction bits

## Categories Tried

| Category | Type | Attempts | Kept | Last Tried |
|----------|------|----------|------|------------|
| malformed-input | test-added | 1 | 31 | iteration 1 |
