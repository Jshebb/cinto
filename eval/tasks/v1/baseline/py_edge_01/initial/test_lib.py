import unittest
from lib import first_element, safe_divide

class TestFirstElement(unittest.TestCase):
    def test_normal(self):
        self.assertEqual(first_element([1, 2, 3]), 1)

    def test_empty_returns_none(self):
        self.assertIsNone(first_element([]))

class TestSafeDivide(unittest.TestCase):
    def test_normal(self):
        self.assertEqual(safe_divide(10, 2), 5.0)

    def test_divide_by_zero_returns_none(self):
        self.assertIsNone(safe_divide(10, 0))

if __name__ == '__main__':
    unittest.main()
