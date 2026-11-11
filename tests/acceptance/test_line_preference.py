import copy
import unittest

from line_preference import validate_cross, validate_restart, validate_same


def cross():
    return dict(first_failure=dict(saved=False, opened=1, code="MEDIA_UNSUPPORTED"),
        second_failure=dict(saved=False, opened=1, code="MEDIA_UNSUPPORTED"), resolve_only=dict(saved=False, opened=2),
        third=dict(route="2", saved=True, frames=3, position=.2, null_coordinates=True, record_count=1),
        second=dict(route="2", fresh=True, dialog=False), other=dict(route="2", fresh=True, dialog=False), no_failed_retry=True,
        ambiguities=[dict(mode=mode, confirmation=True, opens=0, kept=True) for mode in range(1, 12)],
        unavailable_read=dict(notice=True, preserved=True), corruption=dict(notice=True, preserved=True),
        failed_write=dict(notice=True, kept=True, advanced=True, saved=False), recovered_write=True, manual_override=dict(route="0"),
        race=dict(selected=True, context=True, kept=True, pending=True), restart_choice=dict(last_global_is_alternate=True, remembered_source_differs=True),
        unavailable_line=dict(confirmation=True, opens=0, kept=True), source_unavailable=dict(confirmation=True, alternate=True, kept=True),
        storage=dict(records=1, sensitive=False), production_preserved=True, errors=[])


class Oracles(unittest.TestCase):
    def test_valid_cross(self):
        validate_cross(cross())

    def test_resolve_is_not_playback(self):
        for name, key, value in [("resolve_only", "saved", True), ("third", "frames", 0), ("third", "position", 0), ("first_failure", "saved", True)]:
            e = cross()
            e[name][key] = value
            with self.assertRaises(ValueError): validate_cross(e)

    def test_ambiguities_and_secrets(self):
        for key, value in [("confirmation", False), ("opens", 1), ("kept", False)]:
            e = cross()
            e["ambiguities"][0][key] = value
            with self.assertRaises(ValueError): validate_cross(e)
        e = cross()
        e["storage"]["sensitive"] = True
        with self.assertRaises(ValueError): validate_cross(e)

    def test_races_and_write_errors(self):
        for name, key, value in [("race", "selected", False), ("race", "kept", False), ("failed_write", "kept", False), ("failed_write", "saved", True)]:
            e = cross()
            e[name][key] = value
            with self.assertRaises(ValueError): validate_cross(e)
        e = cross()
        e['production_preserved'] = False
        with self.assertRaises(ValueError): validate_cross(e)

    def test_same_source_and_history(self):
        e = dict(cross_restart=dict(route="2", changed_global=True, records=1), same_second=dict(route="2", video=True, fresh=True, old_http=410),
            same_third=dict(route="2", video=True, history_video=True, fresh=True, boundary=True), storage=dict(records=2, sensitive=False), production_preserved=True, errors=[])
        validate_same(e)
        for name, key, value in [("same_second", "old_http", 200), ("same_third", "history_video", False), ("same_second", "fresh", False)]:
            invalid = copy.deepcopy(e)
            invalid[name][key] = value
            with self.assertRaises(ValueError): validate_same(invalid)

    def test_restarts_and_revisions(self):
        e = dict(same_restart=dict(route="2", video=True, saved=True), original_fallback=[dict(mode=mode, route="0", video=True, original_group=True) for mode in (1, 2)],
            revision=dict(changed=True, original_group=True), other_film=dict(different=True, original_group=True), storage=dict(records=2, sensitive=False), production_preserved=True, errors=[])
        validate_restart(e)
        e["revision"]["original_group"] = False
        with self.assertRaises(ValueError): validate_restart(e)


if __name__ == '__main__':
    unittest.main()
