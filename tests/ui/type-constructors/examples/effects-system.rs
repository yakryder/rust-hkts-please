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
    // This would work once Step 8 is fully done (needs unification of ?F ~ Result):
    // let r: Result<i32, String> = Ok(42);
    // let _: Result<i32, String> = identity(r);
    //
    // Currently: ctor_is_identity now correctly treats inference vars as abstract (no crash).
    // Next: Need to solve constructor equations in relate module.
}
