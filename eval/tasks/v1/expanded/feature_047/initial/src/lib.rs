pub struct Entity47 {
    id: u32,
    property_47: String,
}

impl Entity47 {
    pub fn new(id: u32, property_47: String) -> Self {
        Self { id, property_47 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity47::new(1, "test".to_string());
        assert_eq!(e.get_property_47(), "test");
    }
}