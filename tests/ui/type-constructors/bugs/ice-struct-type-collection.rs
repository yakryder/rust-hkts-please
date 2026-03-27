//@ known-bug: unknown
// Parallel ICE to ice-fn-type-collection.rs but for ADT definitions.
// mk_param_from_def fires when type-collecting struct generics.
#![feature(type_constructors)]
#![allow(incomplete_features)]

struct Wrapper<F<_>, A> {
    inner: F<A>,
}

fn main() {}
