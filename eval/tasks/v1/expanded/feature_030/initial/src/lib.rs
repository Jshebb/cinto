pub struct Entity30 {
    id: u32,
    property_30: String,
}

impl Entity30 {
    pub fn new(id: u32, property_30: String) -> Self {
        Self { id, property_30 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity30::new(1, "test".to_string());
        assert_eq!(e.get_property_30(), "test");
    }
}