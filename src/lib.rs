#![no_std]
#![feature(allocator_api)]
#![feature(maybe_uninit_slice)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_write_slice)]
#![feature(new_uninit)]

//! A [trait](Queue) and implementations of non-blocking, infallible queues that support bulk enqueueing and bulk dequeueing via APIs inspired by [ufotofu](https://crates.io/crates/ufotofu).
//!
//! ## Queue Implementations
//!
//! So far, there is only a single implementation: [`Fixed`], which is a heap-allocated ring-buffer of unchanging capacity.
//!
//! Future plans include a queue of static (known at compile-time) capacity that can be used in allocator-less environments, and an elastic queue that grows and shrinks its capacity within certain parameters, to free up memory under low load.

mod fixed;

use core::cmp::min;
use core::mem::MaybeUninit;

pub use fixed::Fixed;

/// A first-in first-out queue. Provides methods for bulk transfer of items similar to [ufotofu](https://crates.io/crates/ufotofu).
pub trait Queue {
    /// The type of items to manage in the queue.
    type Item: Copy;

    /// Return the number of items currently in the queue.
    fn len(&self) -> usize;

    /// Attempt to enqueue an item.
    ///
    /// Will return the item if the queue is full at the time of calling.
    fn enqueue(&mut self, item: Self::Item) -> Option<Self::Item>;

    /// Expose a non-empty slice of memory for the client code to fill with items that should
    /// be enqueued.
    ///
    /// Will return `None` if the queue is full at the time of calling.
    fn enqueue_slots(&mut self) -> Option<&mut [MaybeUninit<Self::Item>]>;

    /// Inform the queue that `amount` many items have been written to the first `amount`
    /// indices of the `enqueue_slots` it has most recently exposed. The semantics must be
    /// equivalent to those of `enqueue` being called `amount` many times with exactly those
    /// items.
    ///
    /// #### Invariants
    ///
    /// Callers must have written into (at least) the `amount` many first `enqueue_slots` that
    /// were most recently exposed. Failure to uphold this invariant may cause undefined behavior.
    ///
    /// #### Safety
    ///
    /// The queue implementation may assume the first `amount` many `enqueue_slots` that were most recently
    /// exposed to contain initialized memory after this call, even if the memory it exposed was
    /// originally uninitialized. Violating the invariants can cause the queue to read undefined
    /// memory, which triggers undefined behavior.
    unsafe fn did_enqueue(&mut self, amount: usize);

    /// Enqueue a non-zero number of items by reading them from a given buffer and returning how
    /// many items were enqueued.
    ///
    /// Will return `0` if the queue is full at the time of calling.
    ///
    /// #### Implementation Notes
    ///
    /// The default implementation orchestrates `enqueue_slots` and `did_enqueue` in a
    /// straightforward manner. Only provide your own implementation if you can do better
    /// than that.
    fn bulk_enqueue(&mut self, buffer: &[Self::Item]) -> usize {
        match self.enqueue_slots() {
            None => 0,
            Some(slots) => {
                let amount = min(slots.len(), buffer.len());
                MaybeUninit::copy_from_slice(&mut slots[..amount], &buffer[..amount]);
                unsafe {
                    self.did_enqueue(amount);
                }

                amount
            }
        }
    }

    /// Attempt to dequeue the next item.
    ///
    /// Will return `None` if the queue is empty at the time of calling.
    fn dequeue(&mut self) -> Option<Self::Item>;

    /// Expose a non-empty slice of items to be dequeued (or an error).
    /// The items in the slice must not have been emitted by `dequeue` before.
    ///
    /// Will return `None` if the queue is empty at the time of calling.
    fn dequeue_slots(&mut self) -> Option<&[Self::Item]>;

    /// Mark `amount` many items as having been dequeued. Future calls to `dequeue` and to
    /// `dequeue_slots` must act as if `dequeue` had been called `amount` many times.
    ///     
    /// #### Invariants
    ///
    /// Callers must not mark items as dequeued that had not previously been exposed by `dequeue_slots`.
    fn did_dequeue(&mut self, amount: usize);

    /// Dequeue a non-zero number of items by writing them into a given buffer and returning how
    /// many items were dequeued.
    ///
    /// Will return `0` if the queue is empty at the time of calling.
    ///
    /// #### Implementation Notes
    ///
    /// The default implementation orchestrates `dequeue_slots` and `did_dequeue` in a
    /// straightforward manner. Only provide your own implementation if you can do better
    /// than that.
    fn bulk_dequeue(&mut self, buffer: &mut [MaybeUninit<Self::Item>]) -> usize {
        match self.dequeue_slots() {
            None => 0,
            Some(slots) => {
                let amount = min(slots.len(), buffer.len());
                MaybeUninit::copy_from_slice(&mut buffer[..amount], &slots[..amount]);
                self.did_dequeue(amount);

                amount
            }
        }
    }
}
