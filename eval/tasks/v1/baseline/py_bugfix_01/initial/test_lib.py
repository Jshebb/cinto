import unittest
from lib import find_max

class TestFindMax(unittest.TestCase):
    def test_basic(self):
        self.assertEqual(find_max([1, 5, 3]), 5)

    def test_last_element_is_max(self):
        self.assertEqual(find_max([1, 2, 9]), 9)

    def test_single_element(self):
        self.assertEqual(find_max([7]), 7)

if __name__ == '__main__':
    unittest.main()
