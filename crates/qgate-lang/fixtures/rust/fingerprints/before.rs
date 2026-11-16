// Fingerprints (design D1 of `add-test-gap-analysis`): which units changed.

fn layout(a: u32) -> u32 {
    a + 1
}

fn comments(a: u32) -> u32 {
    // one
    a
}

fn literal() -> u32 {
    1
}

fn renamed(a: u32) -> u32 {
    a
}

fn typed(a: u32) -> u64 {
    a.into()
}

fn outer() -> u32 {
    fn inner() -> u32 {
        1
    }
    inner()
}

fn closure(v: &[u32]) -> usize {
    v.iter().filter(|x| **x > 1).count()
}

fn in_macro(a: u32) {
    println!("{}", a);
}

#[inline]
fn attribute_before() {}

impl Cart {
    fn total(&self) -> u32 {
        0
    }
}

static TABLE: [u32; 2] = [1, 2];
