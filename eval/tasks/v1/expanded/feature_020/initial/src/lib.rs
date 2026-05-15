pub struct Entity20 {
    id: u32,
    property_20: String,
}

impl Entity20 {
    pub fn new(id: u32, property_20: String) -> Self {
        Self { id, property_20 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity20::new(1, "test".to_string());
        assert_eq!(e.get_property_20(), "test");
    }
}