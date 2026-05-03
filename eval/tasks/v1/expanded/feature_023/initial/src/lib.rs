pub struct Entity23 {
    id: u32,
    property_23: String,
}

impl Entity23 {
    pub fn new(id: u32, property_23: String) -> Self {
        Self { id, property_23 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity23::new(1, "test".to_string());
        assert_eq!(e.get_property_23(), "test");
    }
}