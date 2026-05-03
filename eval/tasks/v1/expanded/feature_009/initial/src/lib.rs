pub struct Entity9 {
    id: u32,
    property_9: String,
}

impl Entity9 {
    pub fn new(id: u32, property_9: String) -> Self {
        Self { id, property_9 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity9::new(1, "test".to_string());
        assert_eq!(e.get_property_9(), "test");
    }
}