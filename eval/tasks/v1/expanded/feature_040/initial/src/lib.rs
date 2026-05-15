pub struct Entity40 {
    id: u32,
    property_40: String,
}

impl Entity40 {
    pub fn new(id: u32, property_40: String) -> Self {
        Self { id, property_40 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity40::new(1, "test".to_string());
        assert_eq!(e.get_property_40(), "test");
    }
}