pub struct Entity3 {
    id: u32,
    property_3: String,
}

impl Entity3 {
    pub fn new(id: u32, property_3: String) -> Self {
        Self { id, property_3 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity3::new(1, "test".to_string());
        assert_eq!(e.get_property_3(), "test");
    }
}