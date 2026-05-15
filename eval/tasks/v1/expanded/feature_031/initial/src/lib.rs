pub struct Entity31 {
    id: u32,
    property_31: String,
}

impl Entity31 {
    pub fn new(id: u32, property_31: String) -> Self {
        Self { id, property_31 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity31::new(1, "test".to_string());
        assert_eq!(e.get_property_31(), "test");
    }
}