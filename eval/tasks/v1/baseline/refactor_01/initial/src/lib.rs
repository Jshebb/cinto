pub fn process_data(data: &[i32]) -> i32 {
    let mut sum = 0;
    for item in data {
        sum += item;
    }
    let average = if data.is_empty() { 0 } else { sum / data.len() as i32 };
    
    // some complex business logic
    let mut result = 0;
    for item in data {
        result += (item - average) * (item - average);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_process() {
        assert_eq!(process_data(&[1, 2, 3, 4, 5]), 10);
    }
}
