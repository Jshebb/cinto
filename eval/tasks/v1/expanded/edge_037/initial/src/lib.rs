pub fn compute_37(x: i32) -> i32 {
    let a = x * 2
    let b = a + 5;
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_compute() {
        assert_eq!(compute_37(2), 9);
    }
}