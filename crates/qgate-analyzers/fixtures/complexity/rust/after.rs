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
        match self.items.len() {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 4,
            _ => 5,
        }
    }

    pub fn report(&self, codes: &[u32]) -> u32 {
        let mut sum = 0;
        for c in codes {
            if *c == 1 || *c == 2 {
                sum += 1;
            } else if *c == 3 && sum > 0 {
                sum += 2;
            }
            match c {
                4 | 5 => sum += 3,
                6 if sum > 9 => sum += 4,
                _ => {}
            }
        }
        while sum > 100 {
            sum -= 10;
        }
        sum
    }
}

impl fmt::Debug for Cart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cart")
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
