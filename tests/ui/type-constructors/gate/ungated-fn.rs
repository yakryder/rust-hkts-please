// No feature attr intentionally.

fn fmap<F<_>, A, B>(fa: F<A>, f: fn(A) -> B) -> F<B> { todo!() }
//~^ ERROR type constructor parameters (`F<_>`) are experimental

fn main() {}
