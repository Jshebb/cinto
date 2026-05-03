pub struct Entity42 {
    id: u32,
    property_42: String,
}

impl Entity42 {
    pub fn new(id: u32, property_42: String) -> Self {
        Self { id, property_42 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity42::new(1, "test".to_string());
        assert_eq!(e.get_property_42(), "test");
    }
}