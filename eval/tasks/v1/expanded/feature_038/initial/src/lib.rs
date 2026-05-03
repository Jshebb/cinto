pub struct Entity38 {
    id: u32,
    property_38: String,
}

impl Entity38 {
    pub fn new(id: u32, property_38: String) -> Self {
        Self { id, property_38 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity38::new(1, "test".to_string());
        assert_eq!(e.get_property_38(), "test");
    }
}