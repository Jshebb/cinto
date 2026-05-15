pub struct Entity50 {
    id: u32,
    property_50: String,
}

impl Entity50 {
    pub fn new(id: u32, property_50: String) -> Self {
        Self { id, property_50 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity50::new(1, "test".to_string());
        assert_eq!(e.get_property_50(), "test");
    }
}