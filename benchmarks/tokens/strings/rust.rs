fn greet(name: &str) -> String { format!("Hello, {}", name.to_uppercase()) }
fn main() { println!("{}", greet("world")); }
