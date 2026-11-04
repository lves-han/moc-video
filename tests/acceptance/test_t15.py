import copy
import unittest

import t15

REF = dict(source_id="p", source_revision="1", data="e")
CONTENT = dict(source_id="p", source_revision="1", data="c")


def good():
    expected = dict(ref=REF, group="g", id="e", content=CONTENT)
    return dict(movie=dict(kind="movie", meta="2021 · Movie", play=True, episode_grid=False, collapsed_chars=281, full_chars=1000, plain_text=True, poster=True),
                series=dict(kind="series", buttons=200, unique=200, videos=200, duplicate_titles=True),
                layouts=[dict(width=960, overflow=False), dict(width=390, overflow=False)],
                sparse=dict(groups=3, poster_placeholder=True, plot=False, meta="Series", switched=[dict(pressed=True, others=1, shown=["a"], expected=["a"])]),
                empty=dict(message=True, play=False, buttons=0), stale=dict(title="Sparse series", data_title="Sparse series"),
                related=dict(cards=1, text="Related title", opened="Sparse series"), errors=[],
                flow=dict(video="v", sent_video="v", first_selection=None, pending=dict(loading=True, disabled=True), cancelled=dict(route="details", title="T", context=None), expected=expected,
                          selection=dict(content_ref=CONTENT, group_id="g", episode_id="e"), confirmed_video="v", context_episode_ref=REF, context_group="g", context_episode="e", context_video="v", hash="#/playback"))


class DetailsEvidence(unittest.TestCase):
    def rejects(self, change):
        value = copy.deepcopy(good())
        change(value)
        with self.assertRaises(ValueError):
            t15.validate_details(value)

    def test_complete_evidence_passes(self):
        t15.validate_details(good())

    def test_selected_episode_must_be_the_played_reference(self):
        self.rejects(lambda e: e["flow"].update(sent_video="other"))
        self.rejects(lambda e: e["flow"].update(context_episode_ref=dict(REF, data="other")))
        self.rejects(lambda e: e["flow"].update(context_group="other"))
        self.rejects(lambda e: e["flow"]["selection"].update(episode_id="other"))

    def test_group_change_and_episode_inventory(self):
        self.rejects(lambda e: e["sparse"]["switched"][0].update(shown=["b"]))
        self.rejects(lambda e: e["sparse"]["switched"][0].update(others=2))
        self.rejects(lambda e: e["series"].update(unique=199))

    def test_stale_response_and_long_plot(self):
        self.rejects(lambda e: e["stale"].update(data_title="Long movie"))
        self.rejects(lambda e: e["movie"].update(plain_text=False))
        self.rejects(lambda e: e["movie"].update(collapsed_chars=1000))

    def test_sparse_empty_and_errors(self):
        self.rejects(lambda e: e["sparse"].update(poster_placeholder=False))
        self.rejects(lambda e: e["empty"].update(buttons=1))
        self.rejects(lambda e: e.update(errors=["boom"]))
