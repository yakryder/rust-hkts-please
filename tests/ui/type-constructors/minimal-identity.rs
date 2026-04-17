// Minimal test: does identity<F<_>, A>(x: F<A>) -> F<A> infer F correctly?
fn identity<F<_>, A>(x: F<A>) -> F<A> {
    x
}

fn main() {
    let opt = Some(42);
    let _: Option<i32> = identity(opt);
}
