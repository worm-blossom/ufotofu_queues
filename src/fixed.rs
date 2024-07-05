extern crate alloc;

use alloc::alloc::{Allocator, Global};
use alloc::boxed::Box;

use core::fmt;
use core::mem::MaybeUninit;

use crate::Queue;

/// A queue holding up to a certain number of items. The capacity is set upon
/// creation and remains fixed. Performs a single heap allocation on creation.
///
/// We will add fallible creation functions based on [`Box::try_new_in`](https://doc.rust-lang.org/nightly/std/boxed/struct.Box.html#method.try_new_in) at a later point, please reach out
/// if you need them for your project.
pub struct Fixed<T, A: Allocator = Global> {
    /// Slice of memory, used as a ring-buffer.
    data: Box<[MaybeUninit<T>], A>,
    /// Read index.
    read: usize,
    /// Amount of valid data.
    amount: usize,
}

impl<T> Fixed<T> {
    /// Create a fixed-capacity queue. Panic if the initial memory allocation fails.
    pub fn new(capacity: usize) -> Self {
        Fixed {
            data: Box::new_uninit_slice(capacity),
            read: 0,
            amount: 0,
        }
    }
}

impl<T, A: Allocator> Fixed<T, A> {
    /// Create a fixed-capacity queue with a given memory allocator. Panic if the initial memory allocation fails.
    pub fn new_in(capacity: usize, alloc: A) -> Self {
        Fixed {
            data: Box::new_uninit_slice_in(capacity, alloc),
            read: 0,
            amount: 0,
        }
    }

    fn is_data_contiguous(&self) -> bool {
        self.read + self.amount < self.capacity()
    }

    /// Return a slice containing the next items that should be read.
    fn readable_slice(&mut self) -> &[MaybeUninit<T>] {
        if self.is_data_contiguous() {
            &self.data[self.read..self.write_to()]
        } else {
            &self.data[self.read..]
        }
    }

    /// Return a slice containing the next slots that should be written to.
    fn writeable_slice(&mut self) -> &mut [MaybeUninit<T>] {
        let capacity = self.capacity();
        let write_to = self.write_to();
        if self.is_data_contiguous() {
            &mut self.data[write_to..capacity]
        } else {
            &mut self.data[write_to..self.read]
        }
    }

    /// Return the capacity with which thise queue was initialised.
    ///
    /// The number of free item slots at any time is `q.capacity() - q.amount()`.
    pub fn capacity(&self) -> usize {
        self.data.len()
    }

    fn write_to(&self) -> usize {
        (self.read + self.amount) % self.capacity()
    }
}

impl<T: Clone, A: Allocator> Fixed<T, A> {
    // For implementing Debug
    fn vec_of_current_items(&self) -> alloc::vec::Vec<T> {
        if self.is_data_contiguous() {
            unsafe {
                MaybeUninit::slice_assume_init_ref(&self.data[self.read..self.write_to()]).to_vec()
            }
        } else {
            // We only work with data thas has been enqueued, so the memory is not uninitialized anymore.
            unsafe {
                let mut ret = MaybeUninit::slice_assume_init_ref(&self.data[self.read..]).to_vec();
                let len_first_slice = ret.len();
                ret.extend_from_slice(MaybeUninit::slice_assume_init_ref(
                    &self.data[0..(self.amount - len_first_slice)],
                ));
                ret
            }
        }
    }
}

impl<T: Copy, A: Allocator> Queue for Fixed<T, A> {
    type Item = T;

    /// Return the number of items in the queue.
    fn len(&self) -> usize {
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

impl<T: Clone + fmt::Debug, A: Allocator> fmt::Debug for Fixed<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fixed")
            .field("capacity", &self.capacity())
            .field("len", &self.amount)
            .field("data", &self.vec_of_current_items())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;

    use super::*;

    #[test]
    fn enqueues_and_dequeues_with_correct_amount() {
        let mut queue: Fixed<u8> = Fixed::new(4);

        assert_eq!(queue.enqueue(7), None);
        assert_eq!(queue.enqueue(21), None);
        assert_eq!(queue.enqueue(196), None);
        assert_eq!(queue.len(), 3);

        assert_eq!(queue.enqueue(233), None);
        assert_eq!(queue.len(), 4);

        // Queue should be first-in, first-out.
        assert_eq!(queue.dequeue(), Some(7));
        assert_eq!(queue.len(), 3);
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

    #[test]
    fn test_debug_impl() {
        let mut queue: Fixed<u8> = Fixed::new(4);

        assert_eq!(queue.enqueue(7), None);
        assert_eq!(queue.enqueue(21), None);
        assert_eq!(queue.enqueue(196), None);
        assert_eq!(format!("{:?}", queue), "Fixed { capacity: 4, len: 3, data: [7, 21, 196] }");

        assert_eq!(queue.dequeue(), Some(7));
        assert_eq!(format!("{:?}", queue), "Fixed { capacity: 4, len: 2, data: [21, 196] }");

        assert_eq!(queue.dequeue(), Some(21));
        assert_eq!(format!("{:?}", queue), "Fixed { capacity: 4, len: 1, data: [196] }");

        assert_eq!(queue.enqueue(33), None);
        assert_eq!(format!("{:?}", queue), "Fixed { capacity: 4, len: 2, data: [196, 33] }");

        assert_eq!(queue.enqueue(17), None);
        assert_eq!(format!("{:?}", queue), "Fixed { capacity: 4, len: 3, data: [196, 33, 17] }");

        assert_eq!(queue.enqueue(200), None);
        assert_eq!(format!("{:?}", queue), "Fixed { capacity: 4, len: 4, data: [196, 33, 17, 200] }");
    }
}
