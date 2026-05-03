pub struct Entity18 {
    id: u32,
    property_18: String,
}

impl Entity18 {
    pub fn new(id: u32, property_18: String) -> Self {
        Self { id, property_18 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity18::new(1, "test".to_string());
        assert_eq!(e.get_property_18(), "test");
    }
}