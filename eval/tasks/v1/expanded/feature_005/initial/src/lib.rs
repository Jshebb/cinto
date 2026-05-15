pub struct Entity5 {
    id: u32,
    property_5: String,
}

impl Entity5 {
    pub fn new(id: u32, property_5: String) -> Self {
        Self { id, property_5 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity5::new(1, "test".to_string());
        assert_eq!(e.get_property_5(), "test");
    }
}