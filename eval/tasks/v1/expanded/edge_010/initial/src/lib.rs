pub fn compute_10(x: i32) -> i32 {
    let a = x * 2
    let b = a + 5;
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_compute() {
        assert_eq!(compute_10(2), 9);
    }
}