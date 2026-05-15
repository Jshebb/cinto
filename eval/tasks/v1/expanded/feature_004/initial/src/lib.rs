pub struct Entity4 {
    id: u32,
    property_4: String,
}

impl Entity4 {
    pub fn new(id: u32, property_4: String) -> Self {
        Self { id, property_4 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity4::new(1, "test".to_string());
        assert_eq!(e.get_property_4(), "test");
    }
}