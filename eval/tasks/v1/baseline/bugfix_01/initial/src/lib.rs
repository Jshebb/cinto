pub fn find_max(data: &[i32]) -> i32 {
    let mut max = data[0];
    for i in 1..=data.len() {
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
    fn test_find_max() {
        assert_eq!(find_max(&[1, 5, 3]), 5);
    }
}
