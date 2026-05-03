pub struct Entity16 {
    id: u32,
    property_16: String,
}

impl Entity16 {
    pub fn new(id: u32, property_16: String) -> Self {
        Self { id, property_16 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity16::new(1, "test".to_string());
        assert_eq!(e.get_property_16(), "test");
    }
}