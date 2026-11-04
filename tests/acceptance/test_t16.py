import copy
import unittest

import t16


def good():
    return dict(title="Show · Season 1 · Episode 1", play_pause=dict(paused_label="Play", playing_label="Pause"), seek=dict(media=1.2, state_ms=1200), volume=dict(media=.37, label=True, persisted=.37),
                speed_options=[.5, 1, 1.25, 1.5, 2], speed={"0.5": .5, "1": 1, "1.25": 1.25, "1.5": 1.5, "2": 2},
                keys=dict(space_plays=True, space_pauses=True, right_delta=.5, left_delta=-.5, up_delta=.05, down_delta=0, muted=True, unmuted=True, input_keeps_keys=True, at=1),
                fullscreen=dict(on=True, off=True, esc_off=True), subtitles=dict(shown="showing", off=True), stream=dict(session_changed=True, same_resolution=True, selected=True),
                hide=dict(concealed=True, restored=True, restored_by_focus=True), resume=dict(position=1.3, paused=True), seek_saved=True,
                select_episode=dict(panelOpen=True, panelClosed=True, current="v1", video="v3", expected="v3", opens=1, title="Show · Season 1 · Episode 3", old_saved=True, old_position=1500, new_rows_at_open=[]),
                auto_off=dict(video="v3", expected="v3", matches=0, completed=True), stop=dict(matches=0, context=None),
                auto_next=dict(video="v4", expected="v4", playing=True, session_bound=True, title="Show · Season 1 · Episode 4"),
                last=dict(next_disabled=True, matches=0, video="v4", expected="v4"), ambiguous=dict(dialog=True, opens=0, session=None, video_elements=1),
                leave_volume=dict(saved=.61), identity=dict(content_id="h1"),
                history=dict(saved=[dict(content_id="h1", episode=1, episode_total=4, source_id="s", title="Show")]), errors=[])


class PlaybackEvidence(unittest.TestCase):
    def rejects(self, change):
        value = copy.deepcopy(good())
        change(value)
        with self.assertRaises(ValueError):
            t16.validate_playback(value)

    def test_complete_evidence_passes(self):
        t16.validate_playback(good())

    def test_controls_must_reach_real_media(self):
        self.rejects(lambda e: e["seek"].update(media=0))
        self.rejects(lambda e: e["volume"].update(media=1))
        self.rejects(lambda e: e.update(speed_options=[.5, 1, 1.5, 2]))
        self.rejects(lambda e: e["speed"].update({"2": 1}))
        self.rejects(lambda e: e["fullscreen"].update(esc_off=False))

    def test_keyboard_respects_inputs(self):
        self.rejects(lambda e: e["keys"].update(input_keeps_keys=False))
        self.rejects(lambda e: e["keys"].update(up_delta=0))
        self.rejects(lambda e: e["keys"].update(right_delta=-.2))
        self.rejects(lambda e: e["hide"].update(restored_by_focus=False))

    def test_eos_rules(self):
        self.rejects(lambda e: e["auto_off"].update(matches=1))
        self.rejects(lambda e: e["stop"].update(matches=1))
        self.rejects(lambda e: e["auto_next"].update(video="v3"))
        self.rejects(lambda e: e["last"].update(next_disabled=False))
        self.rejects(lambda e: e["ambiguous"].update(opens=1))

    def test_history_cannot_cross_episodes(self):
        self.rejects(lambda e: e["select_episode"].update(new_rows_at_open=[1500]))
        self.rejects(lambda e: e["select_episode"].update(old_saved=False))
        self.rejects(lambda e: e.update(seek_saved=False))
        self.rejects(lambda e: e["history"].update(saved=[]))
        self.rejects(lambda e: e["history"]["saved"][0].update(content_id="demo:series:42"))
        self.rejects(lambda e: e["leave_volume"].update(saved=.37))

    def test_resume_and_errors(self):
        self.rejects(lambda e: e["resume"].update(position=0))
        self.rejects(lambda e: e.update(errors=["boom"]))
