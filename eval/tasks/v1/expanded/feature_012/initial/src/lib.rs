pub struct Entity12 {
    id: u32,
    property_12: String,
}

impl Entity12 {
    pub fn new(id: u32, property_12: String) -> Self {
        Self { id, property_12 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity12::new(1, "test".to_string());
        assert_eq!(e.get_property_12(), "test");
    }
}