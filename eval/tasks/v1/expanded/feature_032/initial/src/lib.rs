pub struct Entity32 {
    id: u32,
    property_32: String,
}

impl Entity32 {
    pub fn new(id: u32, property_32: String) -> Self {
        Self { id, property_32 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity32::new(1, "test".to_string());
        assert_eq!(e.get_property_32(), "test");
    }
}