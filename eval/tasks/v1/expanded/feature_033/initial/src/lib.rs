pub struct Entity33 {
    id: u32,
    property_33: String,
}

impl Entity33 {
    pub fn new(id: u32, property_33: String) -> Self {
        Self { id, property_33 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity33::new(1, "test".to_string());
        assert_eq!(e.get_property_33(), "test");
    }
}