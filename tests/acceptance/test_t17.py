import copy
import unittest

import t17


def good():
    return dict(first_run=dict(guidance="g", link="#/setup", opened=dict(route="sources", empty=True, guidance=2)), invalid=dict(saves=0, messages=dict(a="x", b="x", c="x", d="x")),
                added=dict(kinds=["stremio", "maccms"], state="Registration: ready", last="Last request: no failure this session", shown_secret=False), home=dict(cards=2, setup_gone=True),
                edit=dict(kind_locked=True, same_id=True, count=2, provider=True), independent=dict(metadata_kept=True, home_kept=True, persisted=True),
                disabled=dict(state="Registration: disabled", fallback=True, option=False), enabled=True, deleted=dict(kept_after_cancel=True, configs=2),
                failing=dict(message="boom", others_ready=True, selection_kept=True), real_prepare=dict(message="stopped", host_phase="ready", others_ready=True), cloudstream=dict(stimulus="synthetic", target="cloudstream", plugin="SuperVod v7", guest="ready · headless", host_phase="ready", selectable=True),
                interaction=dict(stimulus="synthetic", after_reject=0, rejected_shown=True, started="Synthetic window open", request=dict(source_id="c", source_revision="rev"), cancelled="cancelled", completed="completed"),
                diagnostics=dict(copied_equals_report=True, platform=True, contract=True, engine=True, package=True, full_digest_absent=True, correlation=True, states_separate=True, secret=False, query=False), errors=[])


class SourcesEvidence(unittest.TestCase):
    def rejects(self, change):
        value = copy.deepcopy(good())
        change(value)
        with self.assertRaises(ValueError):
            t17.validate_sources(value)

    def test_complete_evidence_passes(self):
        t17.validate_sources(good())

    def test_synthetic_cloudstream_cannot_be_reported_as_real(self):
        self.rejects(lambda e: e["cloudstream"].update(stimulus="real"))
        self.rejects(lambda e: e["interaction"].update(stimulus="real"))

    def test_secrets_never_reach_diagnostics(self):
        self.rejects(lambda e: e["diagnostics"].update(secret=True))
        self.rejects(lambda e: e["diagnostics"].update(query=True))
        self.rejects(lambda e: e["added"].update(shown_secret=True))
        self.rejects(lambda e: e["invalid"]["messages"].update(a="https://user:hidden@x"))

    def test_invalid_drafts_never_save(self):
        self.rejects(lambda e: e["invalid"].update(saves=1))

    def test_selection_independence_and_interaction_confirmation(self):
        self.rejects(lambda e: e["independent"].update(home_kept=False))
        self.rejects(lambda e: e["interaction"].update(after_reject=1))
        self.rejects(lambda e: e["disabled"].update(option=True))
        self.rejects(lambda e: e["failing"].update(others_ready=False))
        self.rejects(lambda e: e["real_prepare"].update(message=""))
