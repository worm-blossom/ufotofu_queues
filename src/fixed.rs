extern crate alloc;

use alloc::alloc::{Allocator, Global};
use alloc::boxed::Box;
use core::mem::MaybeUninit;

use crate::Queue;

#[derive(Debug)]
pub struct QueueFullError;

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
    // TODO: Can this be `usize` or do we need a `NonZeroUsize`?
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
    type Error = QueueFullError;

    fn get_amount(&self) -> usize {
        self.amount
    }

    fn enqueue(&mut self, item: T) -> Result<(), QueueFullError> {
        if self.amount == self.capacity() {
            Err(QueueFullError)
        } else {
            self.data[self.write_to()].write(item);
            self.amount += 1;

            Ok(())
        }
    }

    fn enqueue_slots(&mut self) -> Result<&mut [MaybeUninit<T>], QueueFullError> {
        // TODO: Can the amount ever be greater than capacity?
        if self.amount >= self.capacity() {
            Err(QueueFullError)
        } else {
            Ok(self.available_fst())
        }
    }

    // TODO: Requires safety documentation.
    unsafe fn did_enqueue(&mut self, amount: usize) -> Result<(), QueueFullError> {
        self.amount += amount;

        Ok(())
    }

    fn dequeue(&mut self) -> Result<T, QueueFullError> {
        if self.amount == 0 {
            Err(QueueFullError)
        } else {
            let previous_read = self.read;
            self.read = (self.read + 1) % self.capacity();
            self.amount -= 1;

            Ok(unsafe { self.data[previous_read].assume_init() })
        }
    }

    fn dequeue_slots(&mut self) -> Result<&[T], QueueFullError> {
        if self.amount == 0 {
            Err(QueueFullError)
        } else {
            Ok(unsafe { MaybeUninit::slice_assume_init_ref(self.readable_fst()) })
        }
    }

    fn did_dequeue(&mut self, amount: usize) -> Result<(), QueueFullError> {
        self.read = (self.read + amount) % self.capacity();
        self.amount -= amount;

        Ok(())
    }
}
