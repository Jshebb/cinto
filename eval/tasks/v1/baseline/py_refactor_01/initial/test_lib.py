import unittest
from lib import process_data, calculate_average

class TestProcessData(unittest.TestCase):
    def test_basic(self):
        result = process_data([1.0, 2.0, 3.0])
        self.assertAlmostEqual(result[0], -1.0)
        self.assertAlmostEqual(result[1], 0.0)
        self.assertAlmostEqual(result[2], 1.0)

class TestCalculateAverage(unittest.TestCase):
    def test_average(self):
        self.assertAlmostEqual(calculate_average([1.0, 2.0, 3.0]), 2.0)

    def test_empty(self):
        self.assertEqual(calculate_average([]), 0.0)

if __name__ == '__main__':
    unittest.main()
