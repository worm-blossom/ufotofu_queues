extern crate alloc;

use alloc::alloc::{Allocator, Global};
use alloc::boxed::Box;

use core::mem::MaybeUninit;

use crate::Queue;

/// A fixed queue error.
#[derive(Debug, PartialEq, Eq)]
pub enum FixedQueueError {
    /// No items are available.
    Empty,
    /// All available capacity is occupied.
    Full,
}

/// A queue holding up to a certain number of items. The capacity is set upon
/// creation and remains fixed. Performs a single heap allocation on creation.
#[derive(Debug)]
pub struct Fixed<T, A: Allocator = Global> {
    /// Slice of memory.
    data: Box<[MaybeUninit<T>], A>,
    /// Read index.
    read: usize,
    /// Amount of valid data.
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

    /// Return a readable slice from the queue, which may or may not be contiguous.
    fn readable_slice(&mut self) -> &[MaybeUninit<T>] {
        if self.is_data_contiguous() {
            &self.data[self.read..self.write_to()]
        } else {
            &self.data[self.read..]
        }
    }

    /// Return a writeable slice from the queue, which may or may not be contiguous.
    fn writeable_slice(&mut self) -> &mut [MaybeUninit<T>] {
        let capacity = self.capacity();
        if self.is_data_contiguous() {
            &mut self.data[self.read + self.amount..capacity]
        } else {
            &mut self.data[(self.read + self.amount) % capacity..self.read]
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

    /// Return the amount of items in the queue.
    fn get_amount(&self) -> usize {
        self.amount
    }

    /// Attempt to enqueue the next item.
    ///
    /// Will return an error if the queue is full at the time of calling.
    fn enqueue(&mut self, item: T) -> Result<(), FixedQueueError> {
        if self.amount == self.capacity() {
            Err(FixedQueueError::Full)
        } else {
            self.data[self.write_to()].write(item);
            self.amount += 1;

            Ok(())
        }
    }

    /// Expose a non-empty slice of memory for the client code to fill with items that should
    /// be enqueued.
    ///
    /// Will return an error if the queue is full at the time of calling.
    fn enqueue_slots(&mut self) -> Result<&mut [MaybeUninit<T>], FixedQueueError> {
        // TODO: Can the amount ever be greater than capacity?
        if self.amount >= self.capacity() {
            Err(FixedQueueError::Full)
        } else {
            Ok(self.writeable_slice())
        }
    }

    /// Instruct the queue to consume the first `amount` many items of the `enqueue_slots`
    /// it has most recently exposed. The semantics must be equivalent to those of `enqueue`
    /// being called `amount` many times with exactly those items.
    ///
    /// #### Invariants
    ///
    /// Callers must have written into (at least) the `amount` many first `enqueue_slots` that
    /// were most recently exposed. Failure to uphold this invariant may cause undefined behavior.
    ///
    /// #### Safety
    ///
    /// Callers may assume the first `amount` many `enqueue_slots` that were most recently
    /// exposed to contain initialized memory after this call, even if the memory it exposed was
    /// originally uninitialized. Violating the invariants can cause the queue to read undefined
    /// memory, which triggers undefined behavior.
    unsafe fn did_enqueue(&mut self, amount: usize) -> Result<(), FixedQueueError> {
        self.amount += amount;

        Ok(())
    }

    /// Attempt to dequeue the next item.
    ///
    /// Will return an error if the queue is empty at the time of calling.
    fn dequeue(&mut self) -> Result<T, FixedQueueError> {
        if self.amount == 0 {
            Err(FixedQueueError::Empty)
        } else {
            let previous_read = self.read;
            // Advance the read index by 1 or reset to 0 if at capacity.
            self.read = (self.read + 1) % self.capacity();
            self.amount -= 1;

            Ok(unsafe { self.data[previous_read].assume_init() })
        }
    }

    /// Expose a non-empty slice of items to be dequeued (or an error).
    /// The items in the slice must not have been emitted by `dequeue` before.
    ///
    /// Will return an error if the queue is empty at the time of calling.
    fn dequeue_slots(&mut self) -> Result<&[T], FixedQueueError> {
        if self.amount == 0 {
            Err(FixedQueueError::Empty)
        } else {
            Ok(unsafe { MaybeUninit::slice_assume_init_ref(self.readable_slice()) })
        }
    }

    /// Mark `amount` many items as having been dequeued. Future calls to `dequeue` and to
    /// `dequeue_slots` must act as if `dequeue` had been called `amount` many times.
    ///
    /// #### Invariants
    ///
    /// Callers must not mark items as dequeued that had not previously been exposed by `dequeue_slots`.
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
    fn bulk_enqueues_and_dequeues_with_correct_amount() {
        let mut queue: Fixed<u8> = Fixed::new(4);
        let mut buf: [MaybeUninit<u8>; 4] = MaybeUninit::uninit_array();

        let amount = queue.bulk_enqueue(b"ufo");
        assert_eq!(amount.unwrap(), 3);

        let amount = queue.bulk_dequeue(&mut buf);
        assert_eq!(amount.unwrap(), 3);
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

    #[test]
    fn errors_on_enqueue_slots_when_none_are_available() {
        // Create a fixed queue that exposes four slots.
        let mut queue: Fixed<u8> = Fixed::new(4);

        // Copy data to two of the available slots and call `did_enqueue`.
        let data = b"tofu";
        let slots = queue.enqueue_slots().unwrap();
        MaybeUninit::copy_from_slice(&mut slots[0..2], &data[0..2]);
        unsafe {
            assert!(queue.did_enqueue(2).is_ok());
        }

        // Copy data to two of the available slots and call `did_enqueue`.
        let slots = queue.enqueue_slots().unwrap();
        MaybeUninit::copy_from_slice(&mut slots[0..2], &data[0..2]);
        unsafe {
            assert!(queue.did_enqueue(2).is_ok());
        }

        // Make a third call to `enqueue_slots` after all available slots have been used.
        assert_eq!(queue.enqueue_slots().unwrap_err(), FixedQueueError::Full);
    }

    #[test]
    fn errors_on_dequeue_slots_when_none_are_available() {
        // Create a fixed queue that exposes four slots.
        let mut queue: Fixed<u8> = Fixed::new(4);

        let data = b"tofu";
        let _amount = queue.bulk_enqueue(data).unwrap();

        let _slots = queue.dequeue_slots().unwrap();
        assert!(queue.did_dequeue(4).is_ok());

        // Make a second call to `dequeue_slots` after all available slots have been used.
        assert_eq!(queue.dequeue_slots().unwrap_err(), FixedQueueError::Empty);
    }
}
