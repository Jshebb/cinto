import unittest
from lib import calculate_average

class TestCalculateAverage(unittest.TestCase):
    def test_normal(self):
        self.assertEqual(calculate_average([1, 2, 3]), 2.0)

    def test_empty_returns_zero(self):
        self.assertEqual(calculate_average([]), 0.0)

    def test_single(self):
        self.assertEqual(calculate_average([5]), 5.0)

if __name__ == '__main__':
    unittest.main()
