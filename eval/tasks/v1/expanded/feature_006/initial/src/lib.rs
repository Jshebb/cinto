pub struct Entity6 {
    id: u32,
    property_6: String,
}

impl Entity6 {
    pub fn new(id: u32, property_6: String) -> Self {
        Self { id, property_6 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity6::new(1, "test".to_string());
        assert_eq!(e.get_property_6(), "test");
    }
}