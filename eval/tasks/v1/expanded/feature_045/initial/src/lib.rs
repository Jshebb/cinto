pub struct Entity45 {
    id: u32,
    property_45: String,
}

impl Entity45 {
    pub fn new(id: u32, property_45: String) -> Self {
        Self { id, property_45 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity45::new(1, "test".to_string());
        assert_eq!(e.get_property_45(), "test");
    }
}