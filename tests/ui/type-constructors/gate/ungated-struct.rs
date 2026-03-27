// No feature attr intentionally.

struct Wrapper<F<_>, A> {
    //~^ ERROR type constructor parameters (`F<_>`) are experimental
    inner: F<A>,
}

fn main() {}
