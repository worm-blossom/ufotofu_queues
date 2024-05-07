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
    fn enqueue(&mut self, item: Self::Item) -> Result<(), Self::Error>;

    /// Expose a non-empty slice of memory for the client code to fill with items that should
    /// be enqueued.
    fn enqueue_slots(&mut self) -> Result<&mut [MaybeUninit<Self::Item>], Self::Error>;

    /// Instruct the queue to consume the first `amount` many items of the `enqueue_slots`
    /// it has most recently exposed. The semantics must be equivalent to those of `enqueue`
    /// being called `amount` many times with exactly those items.
    // TODO: Safety docs.
    unsafe fn did_enqueue(&mut self, amount: usize) -> Result<(), Self::Error>;

    /// Enqueue a non-zero number of items by reading them from a given buffer and returning how
    /// many items were enqueued.
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
    fn dequeue(&mut self) -> Result<Self::Item, Self::Error>;

    /// Expose a non-empty slice of items to be dequeued (or an error).
    /// The items in the slice must not have been emitted by `dequeue` before.
    fn dequeue_slots(&mut self) -> Result<&[Self::Item], Self::Error>;

    /// Mark `amount` many items as having been dequeued. Future calls to `dequeue` and to
    /// `dequeue_slots` must act as if `dequeue` had been called `amount` many times.
    fn did_dequeue(&mut self, amount: usize) -> Result<(), Self::Error>;

    /// Dequeue a non-zero number of items by writing them into a given buffer and returning how
    /// many items were dequeued.
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
