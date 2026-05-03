pub struct Entity2 {
    id: u32,
    property_2: String,
}

impl Entity2 {
    pub fn new(id: u32, property_2: String) -> Self {
        Self { id, property_2 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity2::new(1, "test".to_string());
        assert_eq!(e.get_property_2(), "test");
    }
}