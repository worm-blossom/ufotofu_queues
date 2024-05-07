#![no_main]
use core::num::NonZeroUsize;

use std::collections::VecDeque;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

use ufotofu_queues::fixed::{Fixed, FixedQueueError};
use ufotofu_queues::Queue;

#[derive(Debug, Arbitrary)]
enum Operation<T> {
    Enqueue(T),
    Dequeue,
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
        }
    }
});
