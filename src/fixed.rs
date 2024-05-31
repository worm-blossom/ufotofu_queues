extern crate alloc;

use alloc::alloc::{Allocator, Global};
use alloc::boxed::Box;

use core::mem::MaybeUninit;

use crate::Queue;

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

    /// Return a readable slice from the queue.
    fn readable_slice(&mut self) -> &[MaybeUninit<T>] {
        if self.is_data_contiguous() {
            &self.data[self.read..self.write_to()]
        } else {
            &self.data[self.read..]
        }
    }

    /// Return a writeable slice from the queue.
    fn writeable_slice(&mut self) -> &mut [MaybeUninit<T>] {
        let capacity = self.capacity();
        if self.is_data_contiguous() {
            &mut self.data[self.read + self.amount..capacity]
        } else {
            &mut self.data[(self.read + self.amount) % capacity..self.read]
        }
    }

    pub fn capacity(&self) -> usize {
        self.data.len()
    }

    fn write_to(&self) -> usize {
        (self.read + self.amount) % self.capacity()
    }
}

impl<T: Copy, A: Allocator> Queue for Fixed<T, A> {
    type Item = T;

    /// Return the amount of items in the queue.
    fn amount(&self) -> usize {
        self.amount
    }

    /// Attempt to enqueue the next item.
    ///
    /// Will return the item if the queue is full at the time of calling.
    fn enqueue(&mut self, item: T) -> Option<T> {
        if self.amount == self.capacity() {
            Some(item)
        } else {
            self.data[self.write_to()].write(item);
            self.amount += 1;

            None
        }
    }

    /// Expose a non-empty slice of memory for the client code to fill with items that should
    /// be enqueued.
    ///
    /// Will return `None` if the queue is full at the time of calling.
    fn enqueue_slots(&mut self) -> Option<&mut [MaybeUninit<T>]> {
        if self.amount == self.capacity() {
            None
        } else {
            Some(self.writeable_slice())
        }
    }

    /// Inform the queue that `amount` many items have been written to the first `amount`
    /// indices of the `enqueue_slots` it has most recently exposed.
    ///
    /// #### Invariants
    ///
    /// Callers must have written into (at least) the `amount` many first `enqueue_slots` that
    /// were most recently exposed. Failure to uphold this invariant may cause undefined behavior.
    ///
    /// #### Safety
    ///
    /// The queue will assume the first `amount` many `enqueue_slots` that were most recently
    /// exposed to contain initialized memory after this call, even if the memory it exposed was
    /// originally uninitialized. Violating the invariants will cause the queue to read undefined
    /// memory, which triggers undefined behavior.
    unsafe fn did_enqueue(&mut self, amount: usize) {
        self.amount += amount;
    }

    /// Attempt to dequeue the next item.
    ///
    /// Will return `None` if the queue is empty at the time of calling.
    fn dequeue(&mut self) -> Option<T> {
        if self.amount == 0 {
            None
        } else {
            let previous_read = self.read;
            // Advance the read index by 1 or reset to 0 if at capacity.
            self.read = (self.read + 1) % self.capacity();
            self.amount -= 1;

            Some(unsafe { self.data[previous_read].assume_init() })
        }
    }

    /// Expose a non-empty slice of items to be dequeued.
    ///
    /// Will return `None` if the queue is empty at the time of calling.
    fn dequeue_slots(&mut self) -> Option<&[T]> {
        if self.amount == 0 {
            None
        } else {
            Some(unsafe { MaybeUninit::slice_assume_init_ref(self.readable_slice()) })
        }
    }

    /// Mark `amount` many items as having been dequeued.
    ///
    /// #### Invariants
    ///
    /// Callers must not mark items as dequeued that had not previously been exposed by
    /// `dequeue_slots`.
    fn did_dequeue(&mut self, amount: usize) {
        self.read = (self.read + amount) % self.capacity();
        self.amount -= amount;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueues_and_dequeues_with_correct_amount() {
        let mut queue: Fixed<u8> = Fixed::new(4);

        assert_eq!(queue.enqueue(7), None);
        assert_eq!(queue.enqueue(21), None);
        assert_eq!(queue.enqueue(196), None);
        assert_eq!(queue.amount(), 3);

        assert_eq!(queue.enqueue(233), None);
        assert_eq!(queue.amount(), 4);

        // Queue should be first-in, first-out.
        assert_eq!(queue.dequeue(), Some(7));
        assert_eq!(queue.amount(), 3);
    }

    #[test]
    fn bulk_enqueues_and_dequeues_with_correct_amount() {
        let mut queue: Fixed<u8> = Fixed::new(4);
        let mut buf: [MaybeUninit<u8>; 4] = MaybeUninit::uninit_array();

        let enqueue_amount = queue.bulk_enqueue(b"ufo");
        let dequeue_amount = queue.bulk_dequeue(&mut buf);

        assert_eq!(enqueue_amount, dequeue_amount);
    }

    #[test]
    fn returns_item_on_enqueue_when_queue_is_full() {
        let mut queue: Fixed<u8> = Fixed::new(1);

        assert_eq!(queue.enqueue(7), None);

        assert_eq!(queue.enqueue(0), Some(0))
    }

    #[test]
    fn returns_none_on_dequeue_when_queue_is_empty() {
        let mut queue: Fixed<u8> = Fixed::new(1);

        // Enqueue and then dequeue an item.
        let _ = queue.enqueue(7);
        let _ = queue.dequeue();

        // The queue is now empty.
        assert!(queue.dequeue().is_none());
    }

    #[test]
    fn returnes_none_on_enqueue_slots_when_none_are_available() {
        // Create a fixed queue that exposes four slots.
        let mut queue: Fixed<u8> = Fixed::new(4);

        // Copy data to two of the available slots and call `did_enqueue`.
        let data = b"tofu";
        let slots = queue.enqueue_slots().unwrap();
        MaybeUninit::copy_from_slice(&mut slots[0..2], &data[0..2]);
        unsafe {
            queue.did_enqueue(2);
        }

        // Copy data to two of the available slots and call `did_enqueue`.
        let slots = queue.enqueue_slots().unwrap();
        MaybeUninit::copy_from_slice(&mut slots[0..2], &data[0..2]);
        unsafe {
            queue.did_enqueue(2);
        }

        // Make a third call to `enqueue_slots` after all available slots have been used.
        assert!(queue.enqueue_slots().is_none());
    }

    #[test]
    fn returns_none_on_dequeue_slots_when_none_are_available() {
        // Create a fixed queue that exposes four slots.
        let mut queue: Fixed<u8> = Fixed::new(4);

        let data = b"tofu";
        let _amount = queue.bulk_enqueue(data);

        let _slots = queue.dequeue_slots().unwrap();
        queue.did_dequeue(4);

        // Make a second call to `dequeue_slots` after all available slots have been used.
        assert!(queue.dequeue_slots().is_none());
    }
}
