pub struct Entity24 {
    id: u32,
    property_24: String,
}

impl Entity24 {
    pub fn new(id: u32, property_24: String) -> Self {
        Self { id, property_24 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity24::new(1, "test".to_string());
        assert_eq!(e.get_property_24(), "test");
    }
}