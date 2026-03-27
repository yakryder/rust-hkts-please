//@ compile-flags: -Z parse-crate-root-only
//@ check-pass

// Verifies that multiple `F<_>` params in a single param list parse correctly.

fn compose<F<_>, G<_>, A>(ga: G<A>) -> F<G<A>> { todo!() }

struct BiWrapper<F<_>, G<_>, A> {
    left: F<A>,
    right: G<A>,
}

fn main() {}
