//@ compile-flags: -Z parse-crate-root-only
//@ check-pass

// Verifies that `F<_>` in a function's generic parameter list parses without
// error. This is a parse-only test; no type collection occurs.

fn fmap<F<_>, A, B>(fa: F<A>, f: fn(A) -> B) -> F<B> { todo!() }

fn identity<F<_>, A>(fa: F<A>) -> F<A> { todo!() }

fn main() {}
