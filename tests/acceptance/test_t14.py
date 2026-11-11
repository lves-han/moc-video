import copy
import unittest

import t14


def good():
    desktop = dict(columns=6, poster_width=165, ratio=1.5, overflow=False)
    return dict(skeleton=12, restored=dict(metadata="stremio", playback="maccms"),
                partial=dict(full_page_error=False, order=[dict(index=0, error=False, cards=100), dict(index=1, error=True, cards=0), dict(index=2, error=False, cards=3)]),
                pages=[dict(before=100, kept=100, after=200, requests=1), dict(before=200, kept=200, after=305, requests=1)],
                pagination=dict(cards=305, unique=305, peers_unchanged=True), images=dict(lazy=True, posters=270, unique_urls=270, placeholders=30),
                long_title=dict(title_chars=200, width=160, card_width=165, height=20),
                layouts=[desktop, dict(desktop, columns=4), dict(desktop, columns=3), dict(columns=2, poster_width=170, ratio=1.5, overflow=False)],
                column_sweep=[dict(content_width=width, columns=min(7, max(4, (width + 18) // 168)), poster_width=170) for width in (660, 700, 764, 800, 830, 900, 1000, 1100, 1250, 1400, 1800)],
                retry=dict(recovered=2, still_second=True, featured_kept=True, requests=1, error_cleared=True),
                playback_source=dict(kept_home=True, requests=0, persisted=True),
                expiry=dict(status_queries=1, home_requests=1, page_one_snapshot=True, cards_after=100, error=False),
                empty=dict(text="This source has no content", retry=True, selector=True), failed=dict(retry=True, selector=True), recovered=100, errors=[],
                **{"return": dict(scrolled=700, restored=700, cards_kept=True, requests=0)})


class HomeEvidence(unittest.TestCase):
    def rejects(self, change):
        value = copy.deepcopy(good())
        change(value)
        with self.assertRaises(ValueError):
            t14.validate_home(value)

    def test_complete_evidence_passes(self):
        t14.validate_home(good())

    def test_failed_section_may_not_replace_page_or_peers(self):
        self.rejects(lambda e: e["partial"].update(full_page_error=True))
        self.rejects(lambda e: e["partial"]["order"][1].update(error=False))
        self.rejects(lambda e: e["partial"]["order"][2].update(cards=0))

    def test_pagination_requires_dedup_and_kept_cards(self):
        self.rejects(lambda e: e["pagination"].update(unique=300))
        self.rejects(lambda e: e["pages"][0].update(after=205))
        self.rejects(lambda e: e["pages"][0].update(kept=0))

    def test_grid_outside_four_to_seven_or_overflowing_fails(self):
        self.rejects(lambda e: e["layouts"][0].update(columns=8))
        self.rejects(lambda e: e["layouts"][1].update(columns=3))
        self.rejects(lambda e: e["layouts"][1].update(poster_width=200))
        self.rejects(lambda e: e["layouts"][3].update(overflow=True))
        self.rejects(lambda e: e["column_sweep"][2].update(columns=3))
        self.rejects(lambda e: e["column_sweep"][-1].update(columns=8))
        self.rejects(lambda e: e["column_sweep"][0].update(poster_width=190))
        self.rejects(lambda e: e.update(column_sweep=e["column_sweep"][:3]))

    def test_return_must_restore_without_refetch(self):
        self.rejects(lambda e: e["return"].update(restored=0))
        self.rejects(lambda e: e["return"].update(requests=1))

    def test_playback_source_change_keeps_metadata_home(self):
        self.rejects(lambda e: e["playback_source"].update(kept_home=False))
        self.rejects(lambda e: e["playback_source"].update(requests=1))

    def test_expiry_refreshes_once_with_page_one(self):
        self.rejects(lambda e: e["expiry"].update(status_queries=2))
        self.rejects(lambda e: e["expiry"].update(home_requests=2))
        self.rejects(lambda e: e["expiry"].update(page_one_snapshot=False))

    def test_empty_failed_and_page_errors(self):
        self.rejects(lambda e: e["empty"].update(retry=False))
        self.rejects(lambda e: e["failed"].update(selector=False))
        self.rejects(lambda e: e.update(errors=["boom"]))
