pub struct Entity35 {
    id: u32,
    property_35: String,
}

impl Entity35 {
    pub fn new(id: u32, property_35: String) -> Self {
        Self { id, property_35 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity35::new(1, "test".to_string());
        assert_eq!(e.get_property_35(), "test");
    }
}