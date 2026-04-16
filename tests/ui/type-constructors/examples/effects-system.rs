//@ check-pass
// Documents the current state of HKT support
// ✅ Function signatures with type constructors work
// ❌ Calling/instantiating type constructors needs Step 8 (GenericArgKind::Ctor inference)
#![feature(type_constructors)]
#![allow(incomplete_features)]

// These signatures compile fine - demonstrating parsing and lowering work
fn identity<F<_>, A>(x: F<A>) -> F<A> {
    x
}

fn compose<F<_>, A, B, C>(
    x: F<A>,
    f: fn(A) -> F<B>,
    g: fn(B) -> F<C>,
) -> F<A> {
    x
}

fn bind<F<_>, A, B>(x: F<A>, f: fn(A) -> F<B>) -> F<B> {
    todo!()
}

fn main() {
    // Test with Option (1-ary constructor, matches F<_>):
    let opt: Option<i32> = Some(42);
    let _: Option<i32> = identity(opt);

    // TODO: Result<T, E> (2-ary) requires partial application support (future work)
    // let r: Result<i32, String> = Ok(42);
    // let _: Result<i32, String> = identity(r);
}
