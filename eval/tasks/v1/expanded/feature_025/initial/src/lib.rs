pub struct Entity25 {
    id: u32,
    property_25: String,
}

impl Entity25 {
    pub fn new(id: u32, property_25: String) -> Self {
        Self { id, property_25 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity25::new(1, "test".to_string());
        assert_eq!(e.get_property_25(), "test");
    }
}