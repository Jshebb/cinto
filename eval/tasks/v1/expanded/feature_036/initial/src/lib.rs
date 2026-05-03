pub struct Entity36 {
    id: u32,
    property_36: String,
}

impl Entity36 {
    pub fn new(id: u32, property_36: String) -> Self {
        Self { id, property_36 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity36::new(1, "test".to_string());
        assert_eq!(e.get_property_36(), "test");
    }
}