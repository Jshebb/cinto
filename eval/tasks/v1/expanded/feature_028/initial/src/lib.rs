pub struct Entity28 {
    id: u32,
    property_28: String,
}

impl Entity28 {
    pub fn new(id: u32, property_28: String) -> Self {
        Self { id, property_28 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity28::new(1, "test".to_string());
        assert_eq!(e.get_property_28(), "test");
    }
}