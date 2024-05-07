#![no_std]
#![feature(allocator_api)]
#![feature(maybe_uninit_slice)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_write_slice)]
#![feature(new_uninit)]

pub mod fixed;

use core::cmp::min;
use core::mem::MaybeUninit;

pub trait Queue {
    type Item: Copy;
    type Error;

    /// Return the amount of items in the queue.
    fn get_amount(&self) -> usize;

    /// Attempt to enqueue the next item.
    ///
    /// After this function returns an error, no further functions of this trait may be invoked.
    ///
    /// #### Invariants
    ///
    /// Must not be called after any function of this trait returned an error.
    fn enqueue(&mut self, item: Self::Item) -> Result<(), Self::Error>;

    /// Expose a non-empty slice of memory for the client code to fill with items that should
    /// be enqueued.
    ///
    /// After this function returns an error, no further functions of this trait may be invoked.
    ///
    /// #### Invariants
    ///
    /// Must not be called after any function of this trait has returned an error.
    fn enqueue_slots(&mut self) -> Result<&mut [MaybeUninit<Self::Item>], Self::Error>;

    /// Instruct the queue to consume the first `amount` many items of the `enqueue_slots`
    /// it has most recently exposed. The semantics must be equivalent to those of `enqueue`
    /// being called `amount` many times with exactly those items.
    ///
    /// After this function returns an error, no further functions of this trait may be invoked.
    ///
    /// #### Invariants
    ///
    /// Callers must have written into (at least) the `amount` many first `enqueue_slots` that
    /// were most recently exposed. Failure to uphold this invariant may cause undefined behavior.
    ///
    /// Must not be called after any function of this trait has returned an error.
    ///
    /// #### Safety
    ///
    /// Callers may assume the first `amount` many `enqueue_slots` that were most recently
    /// exposed to contain initialized memory after this call, even if the memory it exposed was
    /// originally uninitialized. Violating the invariants can cause the queue to read undefined
    /// memory, which triggers undefined behavior.
    unsafe fn did_enqueue(&mut self, amount: usize) -> Result<(), Self::Error>;

    /// Enqueue a non-zero number of items by reading them from a given buffer and returning how
    /// many items were enqueued.
    ///
    /// After this function returns an error, no further functions of this trait may be invoked.
    ///
    /// #### Invariants
    ///
    /// Must not be called after any function of this trait has returned an error.
    ///
    /// #### Implementation Notes
    ///
    /// The default implementation orchestrates `enqueue_slots` and `did_enqueue` in a
    /// straightforward manner. Only provide your own implementation if you can do better
    /// than that.
    fn bulk_enqueue(&mut self, buffer: &[Self::Item]) -> Result<usize, Self::Error> {
        let slots = self.enqueue_slots()?;
        let amount = min(slots.len(), buffer.len());
        MaybeUninit::copy_from_slice(&mut slots[..amount], &buffer[..amount]);
        unsafe {
            self.did_enqueue(amount)?;
        }

        Ok(amount)
    }

    /// Attempt to dequeue the next item.
    ///
    /// After this function returns an error, no further functions of this trait may be invoked.
    ///
    /// #### Invariants
    ///
    /// Must not be called after any function of this trait has returned an error.
    fn dequeue(&mut self) -> Result<Self::Item, Self::Error>;

    /// Expose a non-empty slice of items to be dequeued (or an error).
    /// The items in the slice must not have been emitted by `dequeue` before.
    ///
    /// After this function returns an error, no further functions of this trait may be invoked.
    ///
    /// #### Invariants
    ///
    /// Must not be called after any function of this trait has returned an error.
    fn dequeue_slots(&mut self) -> Result<&[Self::Item], Self::Error>;

    /// Mark `amount` many items as having been dequeued. Future calls to `dequeue` and to
    /// `dequeue_slots` must act as if `dequeue` had been called `amount` many times.
    ///     
    /// After this function returns an error, no further functions of this trait may be invoked.
    ///
    /// #### Invariants
    ///
    /// Callers must not mark items as dequeued that had not previously been exposed by `dequeue_slots`.
    ///
    /// Must not be called after any function of this trait has returned an error.
    fn did_dequeue(&mut self, amount: usize) -> Result<(), Self::Error>;

    /// Dequeue a non-zero number of items by writing them into a given buffer and returning how
    /// many items were dequeued.
    ///
    /// After this function returns an error, no further functions of this trait may be invoked.
    ///
    /// #### Invariants
    ///
    /// Must not be called after any function of this trait has returned an error.
    ///
    /// #### Implementation Notes
    ///
    /// The default implementation orchestrates `dequeue_slots` and `did_dequeue` in a
    /// straightforward manner. Only provide your own implementation if you can do better
    /// than that.
    fn bulk_dequeue(
        &mut self,
        buffer: &mut [MaybeUninit<Self::Item>],
    ) -> Result<usize, Self::Error> {
        let slots = self.dequeue_slots()?;
        let amount = min(slots.len(), buffer.len());
        MaybeUninit::copy_from_slice(&mut buffer[..amount], &slots[..amount]);
        self.did_dequeue(amount)?;

        Ok(amount)
    }
}
