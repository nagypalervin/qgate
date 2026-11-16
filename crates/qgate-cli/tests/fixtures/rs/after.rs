pub fn total(discount: u32) -> u32 {
    if discount > 0 {
        return 1;
    }
    if discount > 10 {
        return 2;
    }
    0
}
