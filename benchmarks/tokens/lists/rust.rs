fn inc(x: i64) -> i64 { x + 1 }
fn main() {
    let xs = vec![1, 2, 3];
    println!("{}", xs.into_iter().map(inc).count());
}
