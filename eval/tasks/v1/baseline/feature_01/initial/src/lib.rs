pub struct User {
    id: u32,
    username: String,
}

impl User {
    pub fn new(id: u32, username: String) -> Self {
        Self { id, username }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_username() {
        let user = User::new(1, "alice".to_string());
        assert_eq!(user.username(), "alice");
    }
}
