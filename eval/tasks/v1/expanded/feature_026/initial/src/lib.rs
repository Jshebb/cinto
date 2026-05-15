pub struct Entity26 {
    id: u32,
    property_26: String,
}

impl Entity26 {
    pub fn new(id: u32, property_26: String) -> Self {
        Self { id, property_26 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity26::new(1, "test".to_string());
        assert_eq!(e.get_property_26(), "test");
    }
}