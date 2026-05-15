pub struct Entity22 {
    id: u32,
    property_22: String,
}

impl Entity22 {
    pub fn new(id: u32, property_22: String) -> Self {
        Self { id, property_22 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity22::new(1, "test".to_string());
        assert_eq!(e.get_property_22(), "test");
    }
}