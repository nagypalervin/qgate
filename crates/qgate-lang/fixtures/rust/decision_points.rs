// Decision points (requirement "Rust decision points").

fn plain(n: u32) -> u32 {
    if n > 0 { n } else { 0 }
}

fn if_else_if(a: bool, b: bool) -> u32 {
    if a {
        1
    } else if b {
        2
    } else {
        3
    }
}

fn if_let(o: Option<u32>) -> u32 {
    if let Some(x) = o { x } else { 0 }
}

fn while_loops(mut v: Vec<u32>, mut n: u32) {
    while n > 0 {
        n -= 1;
    }
    while let Some(x) = v.pop() {
        drop(x);
    }
}

fn for_loop(v: &[u32]) {
    for x in v {
        drop(x);
    }
}

fn let_else(o: Option<u32>) -> u32 {
    let Some(x) = o else { return 0 };
    x
}

fn arms(d: u32) -> u32 {
    match d {
        0 => 1,
        n if n < 50 => 2,
        _ => 3,
    }
}

fn binding_arm(d: u32) -> u32 {
    match d {
        0 => 1,
        x => x,
    }
}

fn wildcard_guard(d: u32) -> u32 {
    match d {
        _ if d > 9 => 1,
        _ => 0,
    }
}

fn alternatives(c: char) -> bool {
    match c {
        'a' | 'e' => true,
        _ => false,
    }
}

fn leading_vert(c: char) -> u32 {
    match c {
        | 'a' | 'e' | 'i' => 1,
        _ => 0,
    }
}

fn nested_alternatives(p: (u32, u32)) -> u32 {
    match p {
        (1 | 2, _) => 1,
        _ => 0,
    }
}

fn and_or(a: bool, b: bool, c: bool) -> bool {
    a && b || c
}

fn let_chain(y: Option<u32>, w: Result<u32, ()>) {
    if let Some(x) = y && x > 1 && let Ok(z) = w {
        drop(z);
    }
}

fn question_mark(p: &Path) -> io::Result<u32> {
    let s = fs::read_to_string(p)?;
    let n = s.trim().parse().map_err(io::Error::other)?;
    Ok(n)
}

fn loops_without_conditions(mut n: u32) -> u32 {
    loop {
        n += 1;
        break;
    }
    'outer: loop {
        continue 'outer;
    }
}

fn let_else_and_loops(o: Option<u32>, mut v: Vec<u32>, w: &[u32]) {
    let Some(x) = o else { return };
    while let Some(y) = v.pop() {}
    for z in w {}
    loop {
        break;
    }
}

fn paid(o: &[Order]) -> usize {
    o.iter().filter(|x| x.paid && !x.void).count()
}

fn closures(v: &[u32]) -> Vec<u32> {
    let f = || 1;
    v.iter().map(|x| if *x > 0 { *x } else { f() }).collect()
}

fn outer(a: bool) -> u32 {
    fn inner(b: bool) -> u32 {
        if b { 1 } else { 0 }
    }
    if a { inner(a) } else { 0 }
}

fn in_macros(a: bool, b: bool) {
    assert!(a && b, "both");
    println!("{}", if a { 1 } else { 2 });
}

fn const_inside() -> u32 {
    const LIMIT: u32 = if cfg!(test) { 1 } else { 2 };
    LIMIT
}
