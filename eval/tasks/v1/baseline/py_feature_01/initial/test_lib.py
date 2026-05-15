import unittest
from lib import User

class TestUser(unittest.TestCase):
    def test_get_username(self):
        u = User("alice", "alice@example.com")
        self.assertEqual(u.get_username(), "alice")

    def test_get_email(self):
        u = User("bob", "bob@example.com")
        self.assertEqual(u.get_email(), "bob@example.com")

if __name__ == '__main__':
    unittest.main()
