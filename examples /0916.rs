use std::num::ParseIntError;

fn multiply1(n1_str: &str, n2_str: &str) -> Result<i32, ParseIntError> {
    Ok(n1_str.parse::<i32>().and_then(|n1| {
        n2_str.parse::<i32>().map(|n2| n1 * n2)
    }))
}

fn main() {
    println!("{}", multiply1("25", "4").unwrap());
}

