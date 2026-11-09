"""Mutation probes for TV2's independent serialized-output oracle."""
import copy
import unittest

from tv2 import check_home


class HomeOracleTests(unittest.TestCase):
    def setUp(self):
        self.row = dict(args=dict(input=dict(page=1, filter=dict(genre="Drama", year="2024"))),
            response=dict(data=dict(images={}, page=dict(sections=[dict(kind="movie", scanned_count=100,
                matched_count=1, filters={key: dict(available=False, options=[])
                    for key in ["genre", "year", "language", "region", "sort"]},
                items=[dict(backdrop=None, plot=None, genres=["Drama"], rating=None, year=2024)])]))))

    def test_valid_nullable_fields(self):
        self.assertEqual(check_home(self.row), dict(sections=1))

    def test_wrong_genre_rejected(self):
        self.row["response"]["data"]["page"]["sections"][0]["items"][0]["genres"] = ["Comedy"]
        with self.assertRaises(ValueError): check_home(self.row)

    def test_wrong_year_rejected(self):
        self.row["response"]["data"]["page"]["sections"][0]["items"][0]["year"] = 2023
        with self.assertRaises(ValueError): check_home(self.row)

    def test_missing_and_empty_style_fields_rejected(self):
        for mutate in [lambda c: c.pop("backdrop"), lambda c: c.update(plot=""), lambda c: c.update(genres=[])]:
            row = copy.deepcopy(self.row)
            mutate(row["response"]["data"]["page"]["sections"][0]["items"][0])
            with self.assertRaises(ValueError): check_home(row)

    def test_counts_over_limit_rejected(self):
        self.row["response"]["data"]["page"]["sections"][0]["scanned_count"] = 101
        with self.assertRaises(ValueError): check_home(self.row)

    def test_missing_option_name_rejected(self):
        self.row["response"]["data"]["page"]["sections"][0]["filters"]["genre"] = dict(available=True, options=[dict(value="Drama")])
        with self.assertRaises(ValueError): check_home(self.row)

    def test_disabled_sort_and_region_rejected(self):
        for key in ["sort", "region"]:
            row = copy.deepcopy(self.row)
            row["response"]["data"]["page"]["sections"][0]["filters"][key] = dict(available=True, options=[])
            with self.assertRaises(ValueError): check_home(row)


if __name__ == "__main__": unittest.main()
