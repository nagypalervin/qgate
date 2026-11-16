// Units and their names (requirements "Rust function units", "Rust
// closures count towards their named ancestor", "Rust qualified names").

fn free(n: u32) -> u32 {
    fn nested(m: u32) -> u32 {
        m
    }
    nested(n)
}

pub struct Cart;

impl Cart {
    pub fn new() -> Self {
        Cart
    }

    pub fn total(&self) -> u32 {
        fn helper() -> u32 {
            0
        }
        helper()
    }
}

trait Shape {
    fn declared(&self) -> f64;
    fn area(&self) -> f64 {
        0.0
    }
}

extern "C" {
    fn abs(x: i32) -> i32;
}

static LIMIT: LazyLock<u32> = LazyLock::new(|| if cfg!(test) { 1 } else { 9 });

const fn konst() -> u32 {
    1
}

mod shop {
    pub struct Cart;

    impl Cart {
        fn total(&self) -> u32 {
            0
        }
    }

    impl fmt::Display for Cart {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            Ok(())
        }
    }

    impl fmt::Debug for Cart {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            Ok(())
        }
    }
}

impl<T: Clone> Stack<T> {
    fn push(&mut self, x: T) {}
}

impl From<u8> for Id {
    fn from(v: u8) -> Self {
        Id
    }
}

impl From<HashMap<String, u32>> for Id {
    fn from(v: HashMap<String, u32>) -> Self {
        Id
    }
}

impl<'a> Parser<'a> {
    async fn parse(&mut self) -> Option<&'a str> {
        None
    }
}

impl Total for &Cart {
    fn total(&self) -> u32 {
        0
    }
}

impl<T> Total for Vec<T>
where
    T: Total,
{
    unsafe fn sum(&self) -> u32 {
        0
    }
}

fn outer() {
    struct Local;
    impl Local {
        fn inside(&self) {}
    }
    let f = |x: u32| {
        fn in_closure() {}
        x
    };
}

#[cfg(unix)]
fn home() -> String {
    a()
}

#[cfg(windows)]
fn home() -> String {
    b()
}

macro_rules! twice {
    ($e:expr) => {
        $e + $e
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_two() {}

    mod deeper {
        fn helper() {}
    }
}

mod declared_elsewhere;
