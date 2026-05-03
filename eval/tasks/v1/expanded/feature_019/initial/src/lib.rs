pub struct Entity19 {
    id: u32,
    property_19: String,
}

impl Entity19 {
    pub fn new(id: u32, property_19: String) -> Self {
        Self { id, property_19 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity19::new(1, "test".to_string());
        assert_eq!(e.get_property_19(), "test");
    }
}