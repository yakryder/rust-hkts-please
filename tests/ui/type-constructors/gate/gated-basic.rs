//@ check-pass
// Step 8 milestone: when this graduates to //@ check-pass, the feature works end-to-end.
#![feature(type_constructors)]
#![allow(incomplete_features)]

// This currently ICEs at mk_param_from_def (compiler/rustc_middle/src/ty/context.rs:2313).
// When Step 8 is done, it should type-check cleanly.
fn fmap<F<_>, A, B>(fa: F<A>, f: fn(A) -> B) -> F<B> { todo!() }

fn main() {}
