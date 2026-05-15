pub struct Entity14 {
    id: u32,
    property_14: String,
}

impl Entity14 {
    pub fn new(id: u32, property_14: String) -> Self {
        Self { id, property_14 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity14::new(1, "test".to_string());
        assert_eq!(e.get_property_14(), "test");
    }
}