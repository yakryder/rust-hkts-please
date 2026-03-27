//@ compile-flags: -Z parse-crate-root-only
//@ check-pass

// Verifies that `F<_>` in a struct/enum/type alias definition parses without error.

struct Wrapper<F<_>, A> {
    inner: F<A>,
}

enum Either<F<_>, A, B> {
    Left(F<A>),
    Right(F<B>),
}

type Compose<F<_>, A> = F<A>;

fn main() {}
