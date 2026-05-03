pub struct Entity21 {
    id: u32,
    property_21: String,
}

impl Entity21 {
    pub fn new(id: u32, property_21: String) -> Self {
        Self { id, property_21 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity21::new(1, "test".to_string());
        assert_eq!(e.get_property_21(), "test");
    }
}