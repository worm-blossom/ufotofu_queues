#![no_main]

use core::mem::MaybeUninit;
use core::slice;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

use std::collections::VecDeque;

use ufotofu_queues::fixed::{Fixed, FixedQueueError};
use ufotofu_queues::Queue;

#[derive(Debug, Arbitrary)]
enum Operation<T> {
    Enqueue(T),
    Dequeue,
    BulkEnqueue(Vec<T>),
    BulkDequeue(u8),
}

fn maybe_uninit_slice_mut<T>(slice: &mut [T]) -> &mut [MaybeUninit<T>] {
    let ptr = slice.as_mut_ptr().cast::<MaybeUninit<T>>();
    unsafe { slice::from_raw_parts_mut(ptr, slice.len()) }
}

fuzz_target!(|data: Vec<Operation<u8>>| {
    let capacity = 32;
    let mut control = VecDeque::new();
    let mut test = Fixed::new(capacity);

    for operation in data {
        match operation {
            Operation::Enqueue(item) => {
                let control_result = if control.len() >= capacity {
                    Err(FixedQueueError::Full)
                } else {
                    control.push_back(item.clone());
                    Ok(())
                };
                let test_result = test.enqueue(item.clone());
                assert_eq!(test_result, control_result);
            }
            Operation::Dequeue => {
                let control_result = control.pop_front().ok_or_else(|| FixedQueueError::Empty);
                let test_result = test.dequeue();
                assert_eq!(test_result, control_result);
            }
            Operation::BulkEnqueue(items) => {
                if let Ok(amount) = test.bulk_enqueue(&items) {
                    for (count, item) in items.iter().enumerate() {
                        if count >= amount {
                            break;
                        } else {
                            control.push_back(item.clone());
                        }
                    }
                }
            }
            Operation::BulkDequeue(n) => {
                let n = n as usize;
                if n > 0 {
                    let mut test_buffer = vec![];
                    test_buffer.resize(n, 0_u8);
                    if let Ok(test_result) =
                        test.bulk_dequeue(maybe_uninit_slice_mut(&mut test_buffer))
                    {
                        let mut control_buffer = vec![];
                        for _ in 0..test_result {
                            if let Some(item) = control.pop_front() {
                                control_buffer.push(item.clone());
                            }
                        }

                        assert_eq!(&test_buffer[..test_result], &control_buffer[..test_result]);
                    }
                }
            }
        }
    }
});
