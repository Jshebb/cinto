pub struct Entity43 {
    id: u32,
    property_43: String,
}

impl Entity43 {
    pub fn new(id: u32, property_43: String) -> Self {
        Self { id, property_43 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity43::new(1, "test".to_string());
        assert_eq!(e.get_property_43(), "test");
    }
}