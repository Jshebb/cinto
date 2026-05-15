pub fn compute_46(x: i32) -> i32 {
    let a = x * 2
    let b = a + 5;
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_compute() {
        assert_eq!(compute_46(2), 9);
    }
}