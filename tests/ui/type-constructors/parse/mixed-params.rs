//@ compile-flags: -Z parse-crate-root-only
//@ check-pass

// Verifies that `F<_>` composes freely with lifetime, regular type, and const params.

fn mixed<'a, F<_>, T, const N: usize>(x: F<T>) -> F<T> { todo!() }

struct Complex<'a, F<_>, T, const N: usize> {
    data: F<T>,
    marker: std::marker::PhantomData<&'a ()>,
}

fn main() {}
