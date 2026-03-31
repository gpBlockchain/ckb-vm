pub mod machine_build;
use bytes::Bytes;
use ckb_vm::machine::{DefaultCoreMachine, VERSION1};
use ckb_vm::memory::Memory;
use ckb_vm::snapshot2::{DataSource, Snapshot2Context};
use ckb_vm::{CoreMachine, SparseMemory, SupportMachine, ISA_IMC};

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
