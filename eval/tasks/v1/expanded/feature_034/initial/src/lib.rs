pub struct Entity34 {
    id: u32,
    property_34: String,
}

impl Entity34 {
    pub fn new(id: u32, property_34: String) -> Self {
        Self { id, property_34 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity34::new(1, "test".to_string());
        assert_eq!(e.get_property_34(), "test");
    }
}