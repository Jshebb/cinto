pub struct Entity15 {
    id: u32,
    property_15: String,
}

impl Entity15 {
    pub fn new(id: u32, property_15: String) -> Self {
        Self { id, property_15 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity15::new(1, "test".to_string());
        assert_eq!(e.get_property_15(), "test");
    }
}