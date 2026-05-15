pub struct Entity1 {
    id: u32,
    property_1: String,
}

impl Entity1 {
    pub fn new(id: u32, property_1: String) -> Self {
        Self { id, property_1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity1::new(1, "test".to_string());
        assert_eq!(e.get_property_1(), "test");
    }
}