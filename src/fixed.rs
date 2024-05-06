extern crate alloc;

use alloc::alloc::{Allocator, Global};
use alloc::boxed::Box;
use core::mem::MaybeUninit;

use crate::Queue;

#[derive(Debug, PartialEq, Eq)]
pub enum FixedQueueError {
    //#[error("No items are available")]
    Empty,
    //#[error("All available capacity is occupied")]
    Full,
}

/// A queue holding up to a certain number of items. The capacity is set upon
/// creation and remains fixed. Performs a single heap allocation on creation.
#[derive(Debug)]
pub struct Fixed<T, A: Allocator = Global> {
    data: Box<[MaybeUninit<T>], A>,
    // Reading resumes from this position.
    read: usize,
    // Amount of valid data.
    amount: usize,
}

impl<T> Fixed<T> {
    pub fn new(capacity: usize) -> Self {
        Fixed {
            data: Box::new_uninit_slice(capacity),
            read: 0,
            amount: 0,
        }
    }
}

impl<T, A: Allocator> Fixed<T, A> {
    pub fn new_in(capacity: usize, alloc: A) -> Self {
        Fixed {
            data: Box::new_uninit_slice_in(capacity, alloc),
            read: 0,
            amount: 0,
        }
    }
}

impl<T: Copy, A: Allocator> Fixed<T, A> {
    fn is_data_contiguous(&self) -> bool {
        self.read + self.amount < self.capacity()
    }

    fn available_fst(&mut self) -> &mut [MaybeUninit<T>] {
        let capacity = self.capacity();
        if self.is_data_contiguous() {
            &mut self.data[self.read + self.amount..capacity]
        } else {
            &mut self.data[(self.read + self.amount) % capacity..self.read]
        }
    }

    fn readable_fst(&mut self) -> &[MaybeUninit<T>] {
        if self.is_data_contiguous() {
            &self.data[self.read..self.write_to()]
        } else {
            &self.data[self.read..]
        }
    }

    fn capacity(&self) -> usize {
        self.data.len()
    }

    fn write_to(&self) -> usize {
        (self.read + self.amount) % self.capacity()
    }
}

impl<T: Copy, A: Allocator> Queue for Fixed<T, A> {
    type Item = T;
    type Error = FixedQueueError;

    fn get_amount(&self) -> usize {
        self.amount
    }

    fn enqueue(&mut self, item: T) -> Result<(), FixedQueueError> {
        if self.amount == self.capacity() {
            Err(FixedQueueError::Full)
        } else {
            self.data[self.write_to()].write(item);
            self.amount += 1;

            Ok(())
        }
    }

    fn enqueue_slots(&mut self) -> Result<&mut [MaybeUninit<T>], FixedQueueError> {
        // TODO: Can the amount ever be greater than capacity?
        if self.amount >= self.capacity() {
            Err(FixedQueueError::Full)
        } else {
            Ok(self.available_fst())
        }
    }

    // TODO: Requires safety documentation.
    unsafe fn did_enqueue(&mut self, amount: usize) -> Result<(), FixedQueueError> {
        self.amount += amount;

        Ok(())
    }

    fn dequeue(&mut self) -> Result<T, FixedQueueError> {
        if self.amount == 0 {
            Err(FixedQueueError::Empty)
        } else {
            let previous_read = self.read;
            self.read = (self.read + 1) % self.capacity();
            self.amount -= 1;

            Ok(unsafe { self.data[previous_read].assume_init() })
        }
    }

    fn dequeue_slots(&mut self) -> Result<&[T], FixedQueueError> {
        if self.amount == 0 {
            Err(FixedQueueError::Empty)
        } else {
            Ok(unsafe { MaybeUninit::slice_assume_init_ref(self.readable_fst()) })
        }
    }

    fn did_dequeue(&mut self, amount: usize) -> Result<(), FixedQueueError> {
        self.read = (self.read + amount) % self.capacity();
        self.amount -= amount;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueues_and_dequeues_with_correct_amount() {
        let mut queue: Fixed<u8> = Fixed::new(4);

        queue.enqueue(7).unwrap();
        queue.enqueue(21).unwrap();
        queue.enqueue(196).unwrap();
        assert_eq!(queue.get_amount(), 3);

        queue.enqueue(233).unwrap();
        assert_eq!(queue.get_amount(), 4);

        // Queue should be first-in, first-out.
        assert_eq!(queue.dequeue(), Ok(7));
        assert_eq!(queue.get_amount(), 3);
    }

    #[test]
    fn errors_on_enqueue_when_queue_is_full() {
        let mut queue: Fixed<u8> = Fixed::new(1);

        queue.enqueue(7).unwrap();

        assert_eq!(queue.enqueue(0).unwrap_err(), FixedQueueError::Full);
    }

    #[test]
    fn errors_on_dequeue_when_queue_is_empty() {
        let mut queue: Fixed<u8> = Fixed::new(1);

        queue.enqueue(7).unwrap();
        queue.dequeue().unwrap();

        assert_eq!(queue.dequeue().unwrap_err(), FixedQueueError::Empty);
    }
}
