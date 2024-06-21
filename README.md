# UFOTOFU QUEUES

A [trait](Queue) and implementations of non-blocking, infallible queues that
support bulk enqueueing and bulk dequeueing via APIs inspired by
[ufotofu](https://crates.io/crates/ufotofu).

## Queue Implementations

So far, there is only a single implementation: [`Fixed`], which is a
heap-allocated ring-buffer of unchanging capacity.

Future plans include a queue of static (known at compile-time) capacity that can
be used in allocator-less environments, and an elastic queue that grows and
shrinks its capacity within certain parameters, to free up memory under low
load.
