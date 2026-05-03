pub struct Entity11 {
    id: u32,
    property_11: String,
}

impl Entity11 {
    pub fn new(id: u32, property_11: String) -> Self {
        Self { id, property_11 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity11::new(1, "test".to_string());
        assert_eq!(e.get_property_11(), "test");
    }
}