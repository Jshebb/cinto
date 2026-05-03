pub struct Entity44 {
    id: u32,
    property_44: String,
}

impl Entity44 {
    pub fn new(id: u32, property_44: String) -> Self {
        Self { id, property_44 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity44::new(1, "test".to_string());
        assert_eq!(e.get_property_44(), "test");
    }
}