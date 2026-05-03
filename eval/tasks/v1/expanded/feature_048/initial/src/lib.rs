pub struct Entity48 {
    id: u32,
    property_48: String,
}

impl Entity48 {
    pub fn new(id: u32, property_48: String) -> Self {
        Self { id, property_48 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity48::new(1, "test".to_string());
        assert_eq!(e.get_property_48(), "test");
    }
}