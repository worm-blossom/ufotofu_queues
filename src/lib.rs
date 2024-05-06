#![no_std]
#![feature(allocator_api)]
#![feature(maybe_uninit_slice)]
#![feature(maybe_uninit_write_slice)]
#![feature(new_uninit)]

pub mod fixed;

use core::cmp::min;
use core::mem::MaybeUninit;

pub trait Queue {
    type Item: Copy;
    type Error;

    fn get_amount(&self) -> usize;

    fn enqueue(&mut self, item: Self::Item) -> Result<(), Self::Error>;

    fn enqueue_slots(&mut self) -> Result<&mut [MaybeUninit<Self::Item>], Self::Error>;

    // TODO: Safety docs.
    unsafe fn did_enqueue(&mut self, amount: usize) -> Result<(), Self::Error>;

    fn bulk_enqueue(&mut self, buffer: &[Self::Item]) -> Result<usize, Self::Error> {
        let slots = self.enqueue_slots()?;
        let amount = min(slots.len(), buffer.len());
        MaybeUninit::copy_from_slice(&mut slots[..amount], &buffer[..amount]);
        unsafe {
            self.did_enqueue(amount)?;
        }

        Ok(amount)
    }

    fn dequeue(&mut self) -> Result<Self::Item, Self::Error>;

    fn dequeue_slots(&mut self) -> Result<&[Self::Item], Self::Error>;

    fn did_dequeue(&mut self, amount: usize) -> Result<(), Self::Error>;

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
