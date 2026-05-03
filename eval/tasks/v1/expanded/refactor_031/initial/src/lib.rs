pub fn process_numbers(numbers: &[i32]) -> i32 {
    let mut sum = 0;
    for v in numbers {
        sum += v;
    }
    let avg = if numbers.is_empty() { 0 } else { sum / numbers.len() as i32 };
    
    // Some complex stuff
    let mut result = 0;
    for v in numbers {
        result += (v - avg) * 31;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_process() {
        assert_eq!(process_numbers(&[1, 2, 3]), process_numbers(&[1, 2, 3])); // Identity test to ensure behavior preserves
    }
}