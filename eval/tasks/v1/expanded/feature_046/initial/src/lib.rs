pub struct Entity46 {
    id: u32,
    property_46: String,
}

impl Entity46 {
    pub fn new(id: u32, property_46: String) -> Self {
        Self { id, property_46 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity46::new(1, "test".to_string());
        assert_eq!(e.get_property_46(), "test");
    }
}