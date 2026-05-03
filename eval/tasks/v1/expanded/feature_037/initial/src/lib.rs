pub struct Entity37 {
    id: u32,
    property_37: String,
}

impl Entity37 {
    pub fn new(id: u32, property_37: String) -> Self {
        Self { id, property_37 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity37::new(1, "test".to_string());
        assert_eq!(e.get_property_37(), "test");
    }
}