//@ check-pass
// Fixed in Session 9: type unification now handles Ctor types
// Was parallel ICE to ice-fn-type-collection.rs but for ADT definitions.
#![feature(type_constructors)]
#![allow(incomplete_features)]

struct Wrapper<F<_>, A> {
    inner: F<A>,
}

fn main() {}
