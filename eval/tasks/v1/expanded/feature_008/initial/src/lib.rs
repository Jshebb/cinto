pub struct Entity8 {
    id: u32,
    property_8: String,
}

impl Entity8 {
    pub fn new(id: u32, property_8: String) -> Self {
        Self { id, property_8 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity8::new(1, "test".to_string());
        assert_eq!(e.get_property_8(), "test");
    }
}