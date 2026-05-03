pub struct Entity17 {
    id: u32,
    property_17: String,
}

impl Entity17 {
    pub fn new(id: u32, property_17: String) -> Self {
        Self { id, property_17 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity17::new(1, "test".to_string());
        assert_eq!(e.get_property_17(), "test");
    }
}