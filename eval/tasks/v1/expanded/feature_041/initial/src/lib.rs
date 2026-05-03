pub struct Entity41 {
    id: u32,
    property_41: String,
}

impl Entity41 {
    pub fn new(id: u32, property_41: String) -> Self {
        Self { id, property_41 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity41::new(1, "test".to_string());
        assert_eq!(e.get_property_41(), "test");
    }
}