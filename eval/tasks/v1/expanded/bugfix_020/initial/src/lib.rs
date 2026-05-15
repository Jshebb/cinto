pub fn analyze_20(data: &[i32]) -> i32 {
    if data.is_empty() { return 0; }
    let mut max = data[0];
    for i in 1..=data.len() { // OFF BY ONE BUG
        if data[i] > max {
            max = data[i];
        }
    }
    max
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_analyze() {
        assert_eq!(analyze_20(&[1, 5, 3]), 5);
    }
}