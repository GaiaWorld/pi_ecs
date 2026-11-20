#![cfg(feature = "rc")]
use pi_ecs_storage::column::Column;
use pi_ecs_types::component::ComponentId;
use std::alloc::Layout;

#[test]
fn frozen_column_reads_aligned_and_zero_sized_slots() {
    for layout in [
        Layout::from_size_align(64, 64).unwrap(),
        Layout::from_size_align(0, 128).unwrap(),
    ] {
        let mut column = Column::new(ComponentId(1), layout).unwrap();
        column.prepare(2).unwrap();
        #[repr(align(128))]
        struct Aligned([u8; 64]);
        let mut source = Aligned([19u8; 64]);
        unsafe {
            column.insert_raw(2, source.0.as_mut_ptr(), layout);
            let view = column.bind_view();
            let address = view.read(2).as_non_null().as_ptr();
            assert_eq!(address as usize % layout.align(), 0);
            if layout.size() != 0 {
                assert_eq!(*address, 19);
            }
        }
    }
}

#[test]
fn prepare_preserves_raw_allocation_error() {
    let mut column = Column::new(ComponentId(0), Layout::new::<u64>()).unwrap();
    let error = column.prepare(usize::MAX).unwrap_err();
    assert_eq!(
        error.downcast_ref::<pi_arr::RawArrError>(),
        Some(&pi_arr::RawArrError::IndexOverflow)
    );
    column.prepare(1).unwrap();
    let mut value = 73u64;
    unsafe {
        column.insert_raw(1, (&mut value as *mut u64).cast(), Layout::new::<u64>());
        assert_eq!(*column.get(1).as_non_null().as_ptr().cast::<u64>(), 73);
    }
}
