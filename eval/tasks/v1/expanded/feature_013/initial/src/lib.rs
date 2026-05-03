pub struct Entity13 {
    id: u32,
    property_13: String,
}

impl Entity13 {
    pub fn new(id: u32, property_13: String) -> Self {
        Self { id, property_13 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity13::new(1, "test".to_string());
        assert_eq!(e.get_property_13(), "test");
    }
}