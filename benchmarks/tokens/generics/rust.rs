fn id<T>(x: T) -> T { x }
fn main() {
    println!("{}", id(42));
    println!("{}", id("hello"));
    println!("{}", id(true));
}
