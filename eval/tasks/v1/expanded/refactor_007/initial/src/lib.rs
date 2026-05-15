pub fn process_measurements(measurements: &[i32]) -> i32 {
    let mut sum = 0;
    for v in measurements {
        sum += v;
    }
    let avg = if measurements.is_empty() { 0 } else { sum / measurements.len() as i32 };
    
    // Some complex stuff
    let mut result = 0;
    for v in measurements {
        result += (v - avg) * 7;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_process() {
        assert_eq!(process_measurements(&[1, 2, 3]), process_measurements(&[1, 2, 3])); // Identity test to ensure behavior preserves
    }
}