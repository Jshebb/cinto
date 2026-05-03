pub struct Entity29 {
    id: u32,
    property_29: String,
}

impl Entity29 {
    pub fn new(id: u32, property_29: String) -> Self {
        Self { id, property_29 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity29::new(1, "test".to_string());
        assert_eq!(e.get_property_29(), "test");
    }
}