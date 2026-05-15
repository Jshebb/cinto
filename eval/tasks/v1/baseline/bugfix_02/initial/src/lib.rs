pub fn get_first_char(text: Option<&str>) -> char {
    text.unwrap().chars().next().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_empty() {
        assert_eq!(get_first_char(Some("")), '\0');
    }
    #[test]
    fn test_none() {
        assert_eq!(get_first_char(None), '\0');
    }
}
