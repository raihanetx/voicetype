import unittest

import voicetype
from voicetype.__main__ import main


class SmokeTest(unittest.TestCase):
    def test_version(self) -> None:
        self.assertTrue(voicetype.__version__)

    def test_main_returns_zero(self) -> None:
        self.assertEqual(main(), 0)


if __name__ == "__main__":
    unittest.main()
