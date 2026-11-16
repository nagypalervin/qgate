use std::fmt;

pub struct Cart {
    items: Vec<u32>,
}

impl Cart {
    pub fn total(&self, discount: u32) -> u32 {
        if discount > 0 {
            return 1;
        }
        if discount > 5 {
            return 2;
        }
        0
    }
}

impl fmt::Display for Cart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.items.is_empty() {
            write!(f, "empty")
        } else {
            write!(f, "{} items", self.items.len())
        }
    }
}

impl fmt::Debug for Cart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cart")
    }
}
