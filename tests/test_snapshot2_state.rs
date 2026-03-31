#![cfg(has_asm)]
use bytes::Bytes;
use ckb_vm::elf::{LoadingAction, ProgramMetadata};
use ckb_vm::machine::{DefaultCoreMachine, SupportMachine, VERSION1, VERSION2};
use ckb_vm::memory::{round_page_down, round_page_up, FLAG_DIRTY, FLAG_EXECUTABLE, FLAG_FREEZED};
use ckb_vm::snapshot2::{DataSource, Snapshot2Context};
use ckb_vm::CoreMachine;
use ckb_vm::{
    Memory, Register, SparseMemory, ISA_A, ISA_IMC, ISA_MOP, RISCV_GENERAL_REGISTER_NUMBER,
};
use ckb_vm_definitions::RISCV_PAGESIZE;

const PROGRAM_ID: u64 = 0x1000;
const DATA_ID: u64 = 0x2000;

#[derive(Clone)]
struct MemSource {
    data: std::collections::HashMap<u64, Bytes>,
}

impl MemSource {
    fn new() -> Self {
        let mut data = std::collections::HashMap::new();
        let mut program = vec![0u8; 4096];
        for (i, b) in program.iter_mut().enumerate() {
            *b = (i % 256) as u8;
        }
        data.insert(PROGRAM_ID, Bytes::from(program));
        let content: Vec<u8> = (0..8192).map(|i| (i % 251) as u8).collect();
        data.insert(DATA_ID, Bytes::from(content));
        Self { data }
    }
}

impl DataSource<u64> for MemSource {
    fn load_data(&self, id: &u64, offset: u64, length: u64) -> Option<(Bytes, u64)> {
        let data = self.data.get(id)?;
        let offset = std::cmp::min(offset as usize, data.len());
        let full_size = (data.len() - offset) as u64;
        let real_size = if length > 0 {
            std::cmp::min(full_size, length) as usize
        } else {
            full_size as usize
        };
        Some((data.slice(offset..offset + real_size), full_size))
    }
}

type Core = DefaultCoreMachine<u64, SparseMemory<u64>>;

fn build_machine() -> Core {
    Core::new(ISA_IMC | ISA_A | ISA_MOP, VERSION2, u64::MAX)
}

fn make_loading_action(addr: u64, size: u64, source_start: u64, source_end: u64) -> LoadingAction {
    let aligned_start = round_page_down(addr);
    let padding = addr - aligned_start;
    let aligned_size = round_page_up(size + padding);
    LoadingAction {
        addr: aligned_start,
        size: aligned_size,
        flags: FLAG_EXECUTABLE | FLAG_FREEZED,
        source: source_start..source_end,
        offset_from_addr: padding,
    }
}

#[test]
fn test_snapshot_tracked_page_overwritten_becomes_dirty() {
    let source = MemSource::new();
    let mut ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    let action = make_loading_action(0x10000, RISCV_PAGESIZE as u64, 0, RISCV_PAGESIZE as u64);
    let metadata = ProgramMetadata {
        actions: vec![action],
        entry: 0,
    };
    ctx.mark_program(&mut machine, &metadata, &DATA_ID, 0)
        .unwrap();
    // Write directly to tracked page (not through store_bytes) — this corrupts tracked state
    let custom_data = vec![0xAAu8; RISCV_PAGESIZE];
    machine
        .memory_mut()
        .store_bytes(0x10000, &Bytes::from(custom_data.clone()))
        .unwrap();
    let snapshot = ctx.make_snapshot(&mut machine).unwrap();
    assert!(
        !snapshot.dirty_pages.is_empty(),
        "Directly overwritten tracked page should be in dirty_pages"
    );
    let (addr, _, content) = &snapshot.dirty_pages[0];
    assert_eq!(*addr, 0x10000);
    assert_eq!(content.len(), RISCV_PAGESIZE);
}

#[test]
fn test_snapshot_track_untrack_retrack() {
    let source = MemSource::new();
    let mut ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    let page_addr = 0x20000;
    let action = make_loading_action(page_addr, RISCV_PAGESIZE as u64, 0, RISCV_PAGESIZE as u64);
    let metadata = ProgramMetadata {
        actions: vec![action],
        entry: 0,
    };
    ctx.mark_program(&mut machine, &metadata, &DATA_ID, 0)
        .unwrap();
    ctx.untrack_pages(&mut machine, page_addr, RISCV_PAGESIZE as u64)
        .unwrap();
    ctx.track_pages(
        &mut machine,
        page_addr,
        RISCV_PAGESIZE as u64,
        &DATA_ID,
        4096,
    )
    .unwrap();
    let snapshot = ctx.make_snapshot(&mut machine).unwrap();
    let has_source = snapshot
        .pages_from_source
        .iter()
        .any(|(addr, _, _, _, _)| *addr == page_addr);
    assert!(
        has_source,
        "Re-tracked page should appear in pages_from_source"
    );
}

#[test]
fn test_snapshot_resume_mixed_dirty_and_source() {
    let source = MemSource::new();
    let mut ctx = Snapshot2Context::new(source.clone());
    let mut machine1 = build_machine();
    let action1 = make_loading_action(0x10000, RISCV_PAGESIZE as u64, 0, RISCV_PAGESIZE as u64);
    let action2 = make_loading_action(0x20000, RISCV_PAGESIZE as u64, 0, RISCV_PAGESIZE as u64);
    let metadata = ProgramMetadata {
        actions: vec![action1, action2],
        entry: 0,
    };
    ctx.mark_program(&mut machine1, &metadata, &DATA_ID, 0)
        .unwrap();
    let custom_data = vec![0xAAu8; RISCV_PAGESIZE];
    machine1
        .memory_mut()
        .store_bytes(0x10000, &Bytes::from(custom_data.clone()))
        .unwrap();
    let snapshot = ctx.make_snapshot(&mut machine1).unwrap();
    let mut ctx2 = Snapshot2Context::new(source.clone());
    let mut machine2 = build_machine();
    ctx2.resume(&mut machine2, &snapshot).unwrap();
    let restored = machine2
        .memory_mut()
        .load_bytes(0x10000, RISCV_PAGESIZE as u64)
        .unwrap();
    assert_eq!(&restored[..], &custom_data[..]);
    let source_data = source
        .load_data(&DATA_ID, 0, RISCV_PAGESIZE as u64)
        .unwrap()
        .0;
    let restored2 = machine2
        .memory_mut()
        .load_bytes(0x20000, RISCV_PAGESIZE as u64)
        .unwrap();
    assert_eq!(&restored2[..], &source_data[..]);
}

#[test]
fn test_snapshot_resume_version_mismatch() {
    let source = MemSource::new();
    let ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    let snapshot = ctx.make_snapshot(&mut machine).unwrap();
    let mut machine_wrong = Core::new(ISA_IMC | ISA_A | ISA_MOP, VERSION1, u64::MAX);
    let mut ctx2 = Snapshot2Context::new(source.clone());
    let result = ctx2.resume(&mut machine_wrong, &snapshot);
    assert!(result.is_err());
}

#[test]
fn test_snapshot_store_bytes_spanning_pages() {
    let source = MemSource::new();
    let mut ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    // Use 2 pages (MemSource content is 8192 = 2 * RISCV_PAGESIZE)
    let store_size = 2 * RISCV_PAGESIZE as u64;
    ctx.store_bytes(&mut machine, 0x30000, &DATA_ID, 0, store_size, 0)
        .unwrap();
    let snapshot = ctx.make_snapshot(&mut machine).unwrap();
    // store_bytes re-tracks pages from DataSource, so they're in pages_from_source
    let total_source_bytes: usize = snapshot
        .pages_from_source
        .iter()
        .map(|(_, _, _, _, len)| *len as usize)
        .sum();
    assert_eq!(
        total_source_bytes, store_size as usize,
        "store_bytes should track pages from data source"
    );
}

#[test]
fn test_snapshot_empty_machine() {
    let source = MemSource::new();
    let ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    let snapshot = ctx.make_snapshot(&mut machine).unwrap();
    assert!(snapshot.pages_from_source.is_empty());
    assert!(snapshot.dirty_pages.is_empty());
    assert_eq!(snapshot.registers.len(), RISCV_GENERAL_REGISTER_NUMBER);
}

#[test]
fn test_snapshot_resume_preserves_registers() {
    let source = MemSource::new();
    let ctx = Snapshot2Context::new(source.clone());
    let mut machine1 = build_machine();
    for i in 0..32 {
        machine1.set_register(i, (i as u64) * 0x1000);
    }
    machine1.set_cycles(12345);
    machine1.set_max_cycles(999999);
    let snapshot = ctx.make_snapshot(&mut machine1).unwrap();
    let mut ctx2 = Snapshot2Context::new(source.clone());
    let mut machine2 = build_machine();
    ctx2.resume(&mut machine2, &snapshot).unwrap();
    for i in 0..32 {
        assert_eq!(
            machine2.registers()[i].to_u64(),
            (i as u64) * 0x1000,
            "Register {} mismatch",
            i
        );
    }
    assert_eq!(machine2.cycles(), 12345);
    assert_eq!(machine2.max_cycles(), 999999);
}

#[test]
fn test_snapshot_dirty_page_non_contiguous_not_coalesced() {
    let source = MemSource::new();
    let ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    let data = vec![0xBBu8; RISCV_PAGESIZE];
    machine
        .memory_mut()
        .store_bytes(0x10000, &Bytes::from(data.clone()))
        .unwrap();
    machine
        .memory_mut()
        .store_bytes(0x12000, &Bytes::from(data.clone()))
        .unwrap();
    let snapshot = ctx.make_snapshot(&mut machine).unwrap();
    assert_eq!(
        snapshot.dirty_pages.len(),
        2,
        "Non-contiguous dirty pages should not be coalesced"
    );
}

#[test]
fn test_snapshot_dirty_page_different_flags_not_coalesced() {
    let source = MemSource::new();
    let ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    let data = vec![0xCCu8; RISCV_PAGESIZE];
    machine
        .memory_mut()
        .store_bytes(0x10000, &Bytes::from(data.clone()))
        .unwrap();
    machine
        .memory_mut()
        .store_bytes(0x11000, &Bytes::from(data.clone()))
        .unwrap();
    let page0 = 0x10000 / RISCV_PAGESIZE as u64;
    let page1 = 0x11000 / RISCV_PAGESIZE as u64;
    machine.memory_mut().set_flag(page0, FLAG_DIRTY).unwrap();
    machine
        .memory_mut()
        .set_flag(page1, FLAG_DIRTY | FLAG_EXECUTABLE)
        .unwrap();
    let snapshot = ctx.make_snapshot(&mut machine).unwrap();
    assert!(
        snapshot.dirty_pages.len() >= 2,
        "Consecutive dirty pages with different flags should not be coalesced"
    );
}

#[test]
fn test_snapshot_resume_unaligned_source_page_errors() {
    let source = MemSource::new();
    let ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    let mut snapshot = ctx.make_snapshot(&mut machine).unwrap();
    snapshot
        .pages_from_source
        .push((0x10001, FLAG_EXECUTABLE, DATA_ID, 0, RISCV_PAGESIZE as u64));
    let mut ctx2 = Snapshot2Context::new(source.clone());
    let mut machine2 = build_machine();
    assert!(ctx2.resume(&mut machine2, &snapshot).is_err());
}

#[test]
fn test_snapshot_resume_unaligned_dirty_page_errors() {
    let source = MemSource::new();
    let ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    let mut snapshot = ctx.make_snapshot(&mut machine).unwrap();
    snapshot
        .dirty_pages
        .push((0x10001, FLAG_DIRTY, vec![0u8; RISCV_PAGESIZE]));
    let mut ctx2 = Snapshot2Context::new(source.clone());
    let mut machine2 = build_machine();
    assert!(ctx2.resume(&mut machine2, &snapshot).is_err());
}

#[test]
fn test_snapshot_resume_unaligned_dirty_content_errors() {
    let source = MemSource::new();
    let ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    let mut snapshot = ctx.make_snapshot(&mut machine).unwrap();
    snapshot
        .dirty_pages
        .push((0x10000, FLAG_DIRTY, vec![0u8; RISCV_PAGESIZE + 1]));
    let mut ctx2 = Snapshot2Context::new(source.clone());
    let mut machine2 = build_machine();
    assert!(ctx2.resume(&mut machine2, &snapshot).is_err());
}

#[test]
fn test_snapshot_track_pages_zero_length() {
    let source = MemSource::new();
    let mut ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    ctx.track_pages(&mut machine, 0x10000, 0, &DATA_ID, 0)
        .unwrap();
    let snapshot = ctx.make_snapshot(&mut machine).unwrap();
    assert!(snapshot.pages_from_source.is_empty());
}

#[test]
fn test_snapshot_untrack_pages_zero_length() {
    let source = MemSource::new();
    let mut ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    let action = make_loading_action(0x10000, RISCV_PAGESIZE as u64, 0, RISCV_PAGESIZE as u64);
    let metadata = ProgramMetadata {
        actions: vec![action],
        entry: 0,
    };
    ctx.mark_program(&mut machine, &metadata, &DATA_ID, 0)
        .unwrap();
    ctx.untrack_pages(&mut machine, 0x10000, 0).unwrap();
    let snapshot = ctx.make_snapshot(&mut machine).unwrap();
    assert_eq!(snapshot.pages_from_source.len(), 1);
}

#[test]
fn test_snapshot_after_store_bytes_data_integrity() {
    let source = MemSource::new();
    let mut ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    let store_len = 2 * RISCV_PAGESIZE as u64;
    let (written, _) = ctx
        .store_bytes(&mut machine, 0x40000, &DATA_ID, 0, store_len, 0)
        .unwrap();
    assert_eq!(written, store_len);
    let snapshot = ctx.make_snapshot(&mut machine).unwrap();
    let mut ctx2 = Snapshot2Context::new(source.clone());
    let mut machine2 = build_machine();
    ctx2.resume(&mut machine2, &snapshot).unwrap();
    let original = source.load_data(&DATA_ID, 0, store_len).unwrap().0;
    let restored = machine2
        .memory_mut()
        .load_bytes(0x40000, store_len)
        .unwrap();
    assert_eq!(&original[..], &restored[..]);
}

#[test]
fn test_snapshot_mark_program_then_write() {
    let source = MemSource::new();
    let mut ctx = Snapshot2Context::new(source.clone());
    let mut machine = build_machine();
    let action = make_loading_action(0x50000, RISCV_PAGESIZE as u64, 0, RISCV_PAGESIZE as u64);
    let metadata = ProgramMetadata {
        actions: vec![action],
        entry: 0,
    };
    ctx.mark_program(&mut machine, &metadata, &DATA_ID, 0)
        .unwrap();
    let new_data = vec![0xDDu8; RISCV_PAGESIZE];
    machine
        .memory_mut()
        .store_bytes(0x50000, &Bytes::from(new_data.clone()))
        .unwrap();
    let snapshot = ctx.make_snapshot(&mut machine).unwrap();
    let is_in_source = snapshot
        .pages_from_source
        .iter()
        .any(|(addr, _, _, _, _)| *addr == 0x50000);
    assert!(
        !is_in_source,
        "Modified page should not be in pages_from_source"
    );
    let mut ctx2 = Snapshot2Context::new(source.clone());
    let mut machine2 = build_machine();
    ctx2.resume(&mut machine2, &snapshot).unwrap();
    let restored = machine2
        .memory_mut()
        .load_bytes(0x50000, RISCV_PAGESIZE as u64)
        .unwrap();
    assert_eq!(&restored[..], &new_data[..]);
}
