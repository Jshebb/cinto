pub struct Entity49 {
    id: u32,
    property_49: String,
}

impl Entity49 {
    pub fn new(id: u32, property_49: String) -> Self {
        Self { id, property_49 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity49::new(1, "test".to_string());
        assert_eq!(e.get_property_49(), "test");
    }
}