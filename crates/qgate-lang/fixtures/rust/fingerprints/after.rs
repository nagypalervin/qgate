// Fingerprints (design D1 of `add-test-gap-analysis`): which units changed.
// A comment outside every unit.

fn layout(a: u32) -> u32
{
    a
        + 1
}

fn comments(a: u32) -> u32 {
    /// two
    /* three */
    a
}

fn literal() -> u32 {
    2
}

fn renamed(b: u32) -> u32 {
    b
}

fn typed(a: u32) -> u128 {
    a.into()
}

fn outer() -> u32 {
    fn inner() -> u32 {
        2
    }
    inner()
}

fn closure(v: &[u32]) -> usize {
    v.iter().filter(|x| **x > 2).count()
}

fn in_macro(a: u32) {
    println!("{:?}", a);
}

#[inline(always)]
fn attribute_before() {}

impl Cart {
    fn total(&self) -> u32 {
        0
    }
}

static TABLE: [u32; 2] = [3, 4];
