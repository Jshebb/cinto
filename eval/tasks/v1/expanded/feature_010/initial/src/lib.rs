pub struct Entity10 {
    id: u32,
    property_10: String,
}

impl Entity10 {
    pub fn new(id: u32, property_10: String) -> Self {
        Self { id, property_10 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity10::new(1, "test".to_string());
        assert_eq!(e.get_property_10(), "test");
    }
}