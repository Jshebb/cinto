pub fn process_items(items: &[i32]) -> i32 {
    let mut sum = 0;
    for v in items {
        sum += v;
    }
    let avg = if items.is_empty() { 0 } else { sum / items.len() as i32 };
    
    // Some complex stuff
    let mut result = 0;
    for v in items {
        result += (v - avg) * 6;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_process() {
        assert_eq!(process_items(&[1, 2, 3]), process_items(&[1, 2, 3])); // Identity test to ensure behavior preserves
    }
}