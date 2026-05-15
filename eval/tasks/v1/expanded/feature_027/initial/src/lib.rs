pub struct Entity27 {
    id: u32,
    property_27: String,
}

impl Entity27 {
    pub fn new(id: u32, property_27: String) -> Self {
        Self { id, property_27 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_getter() {
        let e = Entity27::new(1, "test".to_string());
        assert_eq!(e.get_property_27(), "test");
    }
}