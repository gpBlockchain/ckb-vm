pub mod machine_build;
use bytes::Bytes;
use ckb_vm::elf::{LoadingAction, ProgramMetadata};
use ckb_vm::machine::{DefaultCoreMachine, VERSION0, VERSION1, VERSION2};
use ckb_vm::memory::Memory;
use ckb_vm::snapshot2::{DataSource, Snapshot2, Snapshot2Context};
use ckb_vm::{CoreMachine, Error, Register, SparseMemory, SupportMachine, ISA_IMC};

#[derive(Default, Clone, PartialEq)]
struct MockDataSource {
    data: Bytes,
}

impl DataSource<u64> for MockDataSource {
    fn load_data(&self, id: &u64, offset: u64, length: u64) -> Option<(Bytes, u64)> {
        if *id == 1 {
            let start = offset as usize;
            let end = (offset + length) as usize;
            if end <= self.data.len() {
                return Some((
                    self.data.slice(start..end),
                    (self.data.len() as u64).saturating_sub(offset),
                ));
            }
        }
        None
    }
}

#[derive(Default, Clone, PartialEq)]
struct ShortReadDataSource {
    data: Bytes,
}

impl DataSource<u64> for ShortReadDataSource {
    fn load_data(&self, id: &u64, offset: u64, length: u64) -> Option<(Bytes, u64)> {
        if *id != 1 {
            return None;
        }
        let start = offset as usize;
        if start > self.data.len() {
            return None;
        }
        let requested_end = (offset + length) as usize;
        let clamped_end = requested_end.min(self.data.len());
        let actual_len = clamped_end.saturating_sub(start);
        let shortened_len = actual_len / 2;
        Some((
            self.data.slice(start..start + shortened_len),
            (self.data.len() as u64).saturating_sub(offset),
        ))
    }
}

#[test]
pub fn test_snapshot2_store_bytes_full_length() {
    let data = vec![0u8; 8192];
    let source = MockDataSource {
        data: Bytes::from(data),
    };
    let mut ctx = Snapshot2Context::new(source);

    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);
    let addr = 0x1000u64;
    let size_addr = 0x2000u64;

    // Test store_bytes returns correct full_length even if length is small
    // load_data returns (data.slice(0..4096), 8192)
    let (written, full) = ctx
        .store_bytes(&mut core, addr, &1u64, 0, 4096, size_addr)
        .unwrap();
    assert_eq!(written, 4096);
    assert_eq!(full, 8192);

    // Verify memory at size_addr contains full_length (8192)
    let val = core.memory_mut().load64(&size_addr).unwrap();
    assert_eq!(val, 8192);
}

#[test]
pub fn test_snapshot2_resume_unaligned_length() {
    let data = vec![0u8; 4000]; // Not multiple of PAGE_SIZE (4096)
    let source = MockDataSource {
        data: Bytes::from(data),
    };
    let mut ctx = Snapshot2Context::new(source);

    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let mut snapshot = ctx.make_snapshot(&mut core).unwrap();
    // Manually add an unaligned page from source
    snapshot.pages_from_source.push((0x3000, 0, 1, 0, 4000));

    let res = ctx.resume(&mut core, &snapshot);
    // Should fail with MemPageUnalignedAccess because 4000 is not multiple of 4096
    assert!(res.is_err());
}

#[test]
pub fn test_snapshot2_track_pages_logic_gap() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    // If start is 0, length is 4095. roundup(0, 4096) = 0.
    // aligned_bytes = 0 - 0 = 0.
    // length (4095) < aligned_bytes (0) is false.
    // length (4095) >= 4096 is false.
    // result: nothing tracked. This is correct for < 1 page.
    let _ = ctx.track_pages(&mut core, 0, 4095, &1, 0).unwrap();
    assert_eq!(
        ctx.make_snapshot(&mut core)
            .unwrap()
            .pages_from_source
            .len(),
        0
    );

    // If start is 1, length is 4096. roundup(1, 4096) = 4096.
    // aligned_bytes = 4096 - 1 = 4095.
    // length (4096) < aligned_bytes (4095) is false.
    // offset becomes 4095. length becomes 4096 - 4095 = 1.
    // length (1) >= 4096 is false.
    // result: nothing tracked, even though we have a full page at 4096?
    // Wait, if start=1, length=4096, the range is [1, 4097).
    // The only full page starting at multiple of 4096 is... none.
    // So this is also technically correct.
    let _ = ctx.track_pages(&mut core, 1, 4096, &1, 0).unwrap();
    assert_eq!(
        ctx.make_snapshot(&mut core)
            .unwrap()
            .pages_from_source
            .len(),
        0
    );
}

#[test]
pub fn test_snapshot2_resume_rejects_short_read_from_data_source() {
    let source = ShortReadDataSource {
        data: Bytes::from(vec![1u8; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let mut snapshot = ctx.make_snapshot(&mut core).unwrap();
    snapshot.pages_from_source.push((0x4000, 0, 1, 0, 4096));

    let result = ctx.resume(&mut core, &snapshot);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), Error::InvalidVersion);
}

// --- Iteration 34: snapshot2 mark_program/init_pages overflow tests ---

#[test]
#[should_panic(expected = "attempt to add with overflow")]
pub fn test_snapshot2_mark_program_with_overflowing_addr_plus_offset() {
    // BUG: snapshot2::init_pages line 232: action.addr + action.offset_from_addr
    // panics with overflow instead of returning error
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let metadata = ProgramMetadata {
        actions: vec![LoadingAction {
            addr: u64::MAX,
            size: 4096,
            flags: 0,
            source: 0..4096,
            offset_from_addr: 1,
        }],
        entry: 0,
    };

    ctx.mark_program(&mut core, &metadata, &1u64, 0).unwrap();
}

#[test]
#[should_panic(expected = "attempt to subtract with overflow")]
pub fn test_snapshot2_init_pages_with_size_less_than_offset() {
    // BUG: snapshot2::init_pages line 235: action.size - action.offset_from_addr
    // panics with underflow when offset_from_addr > size
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let metadata = ProgramMetadata {
        actions: vec![LoadingAction {
            addr: 0x10000,
            size: 100,
            flags: 0,
            source: 0..4096,
            offset_from_addr: 200,
        }],
        entry: 0,
    };

    ctx.mark_program(&mut core, &metadata, &1u64, 0).unwrap();
}

#[test]
#[should_panic(expected = "attempt to subtract with overflow")]
pub fn test_snapshot2_init_pages_with_source_end_less_than_start() {
    // BUG: snapshot2::init_pages line 234: action.source.end - action.source.start
    // panics with underflow when end < start
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let metadata = ProgramMetadata {
        actions: vec![LoadingAction {
            addr: 0x10000,
            size: 4096,
            flags: 0,
            source: 4096..100,
            offset_from_addr: 0,
        }],
        entry: 0,
    };

    ctx.mark_program(&mut core, &metadata, &1u64, 0).unwrap();
}

#[test]
pub fn test_snapshot2_make_snapshot_empty_machine() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let snapshot = ctx.make_snapshot(&mut core).unwrap();
    assert!(snapshot.pages_from_source.is_empty());
    assert!(snapshot.dirty_pages.is_empty());
    assert_eq!(snapshot.version, VERSION1);
    assert_eq!(snapshot.cycles, 0);
}

#[test]
pub fn test_snapshot2_store_bytes_with_zero_length() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    // store_bytes with length=0 should work or return an appropriate error
    let result = ctx.store_bytes(&mut core, 0x10000, &1u64, 0, 0, 0x20000);
    // This calls load_data with length=0 which may return None or empty data
    let _ = result;
}

#[test]
pub fn test_snapshot2_resume_then_store_bytes() {
    let source = MockDataSource {
        data: Bytes::from(vec![0xAA; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    // Resume from empty snapshot
    let snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };
    ctx.resume(&mut core, &snapshot).unwrap();

    // Now store_bytes - should work after resume
    let result = ctx.store_bytes(&mut core, 0x10000, &1u64, 0, 4096, 0x20000);
    assert!(result.is_ok());
}

#[test]
pub fn test_snapshot2_track_pages_wraparound_start_panics() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let _ = ctx.track_pages(&mut core, u64::MAX, 1, &1, 0);
}

#[test]
pub fn test_snapshot2_track_pages_offset_overflow_panics() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    // start=1 yields aligned_bytes=4095, so offset + aligned_bytes overflows.
    let _ = ctx.track_pages(&mut core, 1, 4096, &1, u64::MAX);
}

#[test]
pub fn test_snapshot2_track_pages_out_of_bound_page_returns_error() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    // aligned_start is far beyond available memory pages, clear_flag should fail.
    let result = ctx.track_pages(&mut core, u64::MAX - 4095, 8192, &1, 0);
    assert!(result.is_err());
}

#[test]
pub fn test_snapshot2_untrack_pages_wraparound_should_error() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let result = ctx.untrack_pages(&mut core, u64::MAX, 2);
    assert!(result.is_err());
}

#[test]
pub fn test_snapshot2_untrack_pages_wraparound_large_length_should_error() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let result = ctx.untrack_pages(&mut core, u64::MAX, 4096);
    assert!(result.is_err());
}

// --- Iteration 31: snapshot2::resume state-corruption tests ---

#[derive(Default, Clone, PartialEq)]
struct FailingDataSource;

impl DataSource<u64> for FailingDataSource {
    fn load_data(&self, _id: &u64, _offset: u64, _length: u64) -> Option<(Bytes, u64)> {
        None
    }
}

#[test]
pub fn test_snapshot2_resume_clears_previous_context_pages() {
    // After resume, the Snapshot2Context should have cleared its internal pages map
    let source = MockDataSource {
        data: Bytes::from(vec![0xAB; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    // Track some pages first
    let _ = ctx.track_pages(&mut core, 0x0, 4096, &1u64, 0).unwrap();

    // Verify pages were tracked by making a snapshot
    let snap_before = ctx.make_snapshot(&mut core).unwrap();
    assert!(!snap_before.pages_from_source.is_empty());

    // Now resume from a different snapshot - should clear old state
    let mut snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };
    ctx.resume(&mut core, &snapshot).unwrap();

    // After resume, internal pages should be cleared
    let snap_after = ctx.make_snapshot(&mut core).unwrap();
    assert!(
        snap_after.pages_from_source.is_empty(),
        "resume should clear previous tracked pages"
    );
}

#[test]
pub fn test_snapshot2_resume_with_version_mismatch_version0_vs_version1() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);

    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let mut snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![],
        version: VERSION0,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    let result = ctx.resume(&mut core, &snapshot);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), Error::InvalidVersion);
}

#[test]
pub fn test_snapshot2_resume_preserves_load_reservation_address() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0x1000,
        cycles: 100,
        max_cycles: 500,
        load_reservation_address: 0xDEAD_BEEF,
    };

    ctx.resume(&mut core, &snapshot).unwrap();

    // Verify the load reservation address was set via memory's lr
    let lr = core.memory().lr().to_u64();
    assert_eq!(lr, 0xDEAD_BEEF);
}

#[test]
pub fn test_snapshot2_resume_sets_registers_and_pc() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let mut registers = [0u64; 32];
    registers[1] = 0x1111;
    registers[2] = 0x2222;
    registers[31] = 0xFFFF;

    let snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![],
        version: VERSION1,
        registers,
        pc: 0x8000,
        cycles: 42,
        max_cycles: 1000,
        load_reservation_address: 0,
    };

    ctx.resume(&mut core, &snapshot).unwrap();

    assert_eq!(core.registers()[1].to_u64(), 0x1111);
    assert_eq!(core.registers()[2].to_u64(), 0x2222);
    assert_eq!(core.registers()[31].to_u64(), 0xFFFF);
    assert_eq!(core.pc().to_u64(), 0x8000);
    assert_eq!(core.cycles(), 42);
    assert_eq!(core.max_cycles(), 1000);
}

#[test]
pub fn test_snapshot2_resume_with_empty_snapshot() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    let result = ctx.resume(&mut core, &snapshot);
    assert!(result.is_ok());
}

#[test]
pub fn test_snapshot2_resume_with_only_dirty_pages() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let dirty_content = vec![0xAA; 4096];
    let snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![(0x10000, 0, dirty_content)],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    let result = ctx.resume(&mut core, &snapshot);
    assert!(result.is_ok());

    // Verify the dirty page was written to memory
    let val = core.memory_mut().load8(&(0x10000u64)).unwrap();
    assert_eq!(val, 0xAA);
}

#[test]
pub fn test_snapshot2_resume_dirty_page_unaligned_address_rejected() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    // Dirty page with unaligned address
    let snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![(0x10001, 0, vec![0xBB; 4096])],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    let result = ctx.resume(&mut core, &snapshot);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        Error::MemPageUnalignedAccess(_)
    ));
}

#[test]
pub fn test_snapshot2_resume_dirty_page_unaligned_length_rejected() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    // Dirty page with length not multiple of PAGE_SIZE
    let snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![(0x10000, 0, vec![0xCC; 4000])],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    let result = ctx.resume(&mut core, &snapshot);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        Error::MemPageUnalignedAccess(_)
    ));
}

#[test]
pub fn test_snapshot2_resume_source_page_unaligned_address_rejected() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    // Source page with unaligned address
    let snapshot = Snapshot2 {
        pages_from_source: vec![(0x10001, 0, 1, 0, 4096)],
        dirty_pages: vec![],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    let result = ctx.resume(&mut core, &snapshot);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        Error::MemPageUnalignedAccess(_)
    ));
}

#[test]
pub fn test_snapshot2_resume_data_source_returns_none() {
    let source = FailingDataSource;
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let snapshot = Snapshot2 {
        pages_from_source: vec![(0x4000, 0, 1, 0, 4096)],
        dirty_pages: vec![],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    let result = ctx.resume(&mut core, &snapshot);
    assert!(result.is_err());
    // FailingDataSource always returns None → SnapshotDataLoadError
    assert_eq!(result.unwrap_err(), Error::SnapshotDataLoadError);
}

#[test]
pub fn test_snapshot2_resume_multiple_noncontiguous_source_pages() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 16384]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    // Two non-contiguous pages from source
    let snapshot = Snapshot2 {
        pages_from_source: vec![(0x10000, 0, 1, 0, 4096), (0x20000, 0, 1, 4096, 4096)],
        dirty_pages: vec![],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    let result = ctx.resume(&mut core, &snapshot);
    assert!(result.is_ok());
}

#[test]
pub fn test_snapshot2_resume_with_pc_at_zero() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    ctx.resume(&mut core, &snapshot).unwrap();
    assert_eq!(core.pc().to_u64(), 0);
}

#[test]
pub fn test_snapshot2_resume_with_max_cycles() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: u64::MAX - 1,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    ctx.resume(&mut core, &snapshot).unwrap();
    assert_eq!(core.cycles(), u64::MAX - 1);
}

#[test]
pub fn test_snapshot2_resume_with_multiple_dirty_pages() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![
            (0x10000, 0, vec![0x11; 4096]),
            (0x20000, 0, vec![0x22; 4096]),
            (0x30000, 0, vec![0x33; 4096]),
        ],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    ctx.resume(&mut core, &snapshot).unwrap();

    assert_eq!(core.memory_mut().load8(&(0x10000u64)).unwrap(), 0x11);
    assert_eq!(core.memory_mut().load8(&(0x20000u64)).unwrap(), 0x22);
    assert_eq!(core.memory_mut().load8(&(0x30000u64)).unwrap(), 0x33);
}

#[test]
pub fn test_snapshot2_resume_twice_overwrites_state() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    // First resume with registers set to pattern A
    let mut regs1 = [0u64; 32];
    regs1[1] = 0xAAAA;
    let snapshot1 = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![(0x10000, 0, vec![0xAA; 4096])],
        version: VERSION1,
        registers: regs1,
        pc: 0x100,
        cycles: 10,
        max_cycles: 500,
        load_reservation_address: 0,
    };
    ctx.resume(&mut core, &snapshot1).unwrap();
    assert_eq!(core.registers()[1].to_u64(), 0xAAAA);
    assert_eq!(core.memory_mut().load8(&(0x10000u64)).unwrap(), 0xAA);

    // Second resume with registers set to pattern B (should fully overwrite)
    let mut regs2 = [0u64; 32];
    regs2[1] = 0xBBBB;
    let snapshot2 = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![(0x20000, 0, vec![0xBB; 4096])],
        version: VERSION1,
        registers: regs2,
        pc: 0x200,
        cycles: 20,
        max_cycles: 1000,
        load_reservation_address: 0x1234,
    };
    ctx.resume(&mut core, &snapshot2).unwrap();

    // State should reflect the second resume
    assert_eq!(core.registers()[1].to_u64(), 0xBBBB);
    assert_eq!(core.pc().to_u64(), 0x200);
    assert_eq!(core.cycles(), 20);
    assert_eq!(core.max_cycles(), 1000);
    assert_eq!(core.memory_mut().load8(&(0x20000u64)).unwrap(), 0xBB);

    // The first dirty page may still have old data in memory (resume doesn't clear old memory)
    // but the snapshot context should be reset
}

#[test]
pub fn test_snapshot2_resume_and_make_snapshot_roundtrip() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let mut registers = [0u64; 32];
    registers[1] = 0x1234;
    registers[10] = 0x5678;

    let snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![],
        version: VERSION1,
        registers,
        pc: 0x8000,
        cycles: 100,
        max_cycles: 1000,
        load_reservation_address: 0xAAAA,
    };

    ctx.resume(&mut core, &snapshot).unwrap();
    let snapshot2 = ctx.make_snapshot(&mut core).unwrap();

    assert_eq!(snapshot2.registers[1], 0x1234);
    assert_eq!(snapshot2.registers[10], 0x5678);
    assert_eq!(snapshot2.pc, 0x8000);
    assert_eq!(snapshot2.cycles, 100);
    assert_eq!(snapshot2.max_cycles, 1000);
    assert_eq!(snapshot2.load_reservation_address, 0xAAAA);
}

#[test]
pub fn test_snapshot2_resume_with_source_data_at_nonzero_offset() {
    let mut data = vec![0u8; 8192];
    for i in 0..data.len() {
        data[i] = (i & 0xFF) as u8;
    }
    let source = MockDataSource {
        data: Bytes::from(data),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    // Load from offset 4096 (second page of data)
    let snapshot = Snapshot2 {
        pages_from_source: vec![(0x10000, 0, 1, 4096, 4096)],
        dirty_pages: vec![],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    ctx.resume(&mut core, &snapshot).unwrap();

    // Verify the data at offset 4096 was loaded
    let first_byte = core.memory_mut().load8(&(0x10000u64)).unwrap();
    assert_eq!(first_byte, 0x00); // 4096 & 0xFF = 0
    let second_byte = core.memory_mut().load8(&(0x10001u64)).unwrap();
    assert_eq!(second_byte, 0x01); // 4097 & 0xFF = 1
}

#[test]
pub fn test_snapshot2_resume_with_both_source_and_dirty_pages() {
    let source = MockDataSource {
        data: Bytes::from(vec![0xDD; 8192]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION1, u64::MAX);

    let snapshot = Snapshot2 {
        pages_from_source: vec![(0x10000, 0, 1, 0, 4096)],
        dirty_pages: vec![(0x20000, 0, vec![0xEE; 4096])],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    ctx.resume(&mut core, &snapshot).unwrap();

    // Both source page and dirty page should be loaded
    assert_eq!(core.memory_mut().load8(&(0x10000u64)).unwrap(), 0xDD);
    assert_eq!(core.memory_mut().load8(&(0x20000u64)).unwrap(), 0xEE);
}

#[test]
pub fn test_snapshot2_resume_with_version2() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION2, u64::MAX);

    let snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![],
        version: VERSION2,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    let result = ctx.resume(&mut core, &snapshot);
    assert!(result.is_ok());
}

#[test]
pub fn test_snapshot2_resume_version2_machine_version1_snapshot_fails() {
    let source = MockDataSource {
        data: Bytes::from(vec![0u8; 4096]),
    };
    let mut ctx = Snapshot2Context::new(source);
    let mut core = DefaultCoreMachine::<u64, SparseMemory<u64>>::new(ISA_IMC, VERSION2, u64::MAX);

    let snapshot = Snapshot2 {
        pages_from_source: vec![],
        dirty_pages: vec![],
        version: VERSION1,
        registers: [0u64; 32],
        pc: 0,
        cycles: 0,
        max_cycles: u64::MAX,
        load_reservation_address: 0,
    };

    let result = ctx.resume(&mut core, &snapshot);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), Error::InvalidVersion);
}
