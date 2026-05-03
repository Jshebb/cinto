pub struct Entity7 {
    id: u32,
    property_7: String,
}

impl Entity7 {
    pub fn new(id: u32, property_7: String) -> Self {
        Self { id, property_7 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity7::new(1, "test".to_string());
        assert_eq!(e.get_property_7(), "test");
    }
}