# Bug-Hunt Context

## Project Understanding

CKB-VM is Nervos CKB's RISC-V virtual machine implementation in Rust. It supports multiple execution modes (interpreter, ASM), multiple memory backends (flat, sparse, WXORX), ELF binary loading, snapshot/resume, and RISC-V extensions (M/A/B/V). The VM executes RISC-V 64-bit instructions with cycle counting for consensus.

## Test Coverage Gaps

1. **ELF parsing edge cases** — malformed ELF binaries, truncated headers, invalid section flags
2. **Memory boundary conditions** — page boundary crossing loads/stores, zero-length operations, maximum address access
3. **Instruction decoding** — invalid opcodes, truncated instructions, malformed encoding fields
4. **Snapshot/resume** — corrupted snapshots, partial resume data, memory state consistency
5. **Stack initialization** — oversized arguments, empty args, null bytes in strings
6. **WXorX memory permissions** — write-then-execute violations, permission edge cases
7. **ASM memory checks** — writable/executable/inited checks with boundary addresses
8. **Bits utilities** — roundup/rounddown with non-power-of-2 inputs, zero, max values
9. **Cost model** — complete opcode coverage, unknown opcode handling
10. **Error propagation** — ensure all error paths are exercised and properly typed

## Known Bugs

(None yet — baseline established: 172 tests passing, 0 failing)

## What Works

- Basic VM execution with both Rust and ASM backends
- RISC-V instruction execution across versions (v0, v1, v2)
- Memory load/store operations in common cases
- ELF loading for well-formed binaries
- Snapshot creation and resume
- MOP (macro-operation fusion) instructions
- A/B extensions basic functionality

## Ideas Backlog — Tests to Write

1. **Boundary tests for memory operations** — addresses at page boundaries, max u64, zero
2. **Malformed ELF inputs** — truncated files, invalid magic, corrupted headers
3. **Invalid instruction encodings** — unknown opcodes, truncated instructions
4. **Bits utility edge cases** — zero inputs, non-power-of-2, overflow conditions
5. **Stack init with edge cases** — empty args, huge args, null pointers
6. **WXorX permission violations** — write then execute attempts
7. **Snapshot corruption** — modified snapshot data, missing fields
8. **Cycle limit edge cases** — exactly at limit, one below, overflow
9. **Memory page flag operations** — boundary pages, concurrent flag changes
10. **Error type conversions** — all error variants from external crates

## Categories Tried

| Category | Type | Attempts | Kept | Last Tried |
|----------|------|----------|------|------------|
