"""T0.16 playback UI acceptance: production App/PlaybackView against a real Host and real media in a packaged WKWebView."""
import argparse
import json
import os
import re
from pathlib import Path

from t00 import require
from t10 import resources
import t12
import t14

ROOT = Path(__file__).resolve().parents[2]
IDS = ["U03", "U09", "R23", "R25", "R32", "R42", "R67", "REQ-T16"]


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.16", "T16", IDS


class Bridge(t12.Bridge):
    def finish(self):
        self.invoke(dict(operation="finish"))
        self.process.wait(timeout=30)
        self.reader.join(timeout=5)
        self.record["exit"] = self.process.returncode
        require(self.process.returncode == 0 and not self.reader.is_alive(), "Real Host bridge did not finish cleanly")
        vectors = [json.loads(line.split("T12_VECTOR:", 1)[1]) for line in self.lines if "T12_VECTOR:" in line]
        require(len(vectors) == 1 and vectors[0]["opens"] > 0 and vectors[0]["closed"] is True and vectors[0]["active_requests"] == 0, "Real Host playback evidence missing")
        require("test result: ok. 1 passed; 0 failed; 0 ignored;" in "".join(self.lines), "Actual named native test did not pass")
        require(not self.run.observed.get("host_log_sanitized"), "Native output contained credentials; stored log was sanitized")
        self.run.observed["host"] = vectors[0]


def validate_playback(e):
    require("Season 1" in e["title"] and "Episode 1" in e["title"], "The player must name its group and episode")
    p = e["play_pause"]
    require(p["paused_label"] == "Play" and p["playing_label"] == "Pause", "The play control must follow the real pipeline")
    s = e["seek"]
    require(abs(s["media"] - 1.2) < .06 and abs(s["state_ms"] - s["media"] * 1000) < 300, "Seek must reach real media")
    v = e["volume"]
    require(abs(v["media"] - .37) < .001 and v["label"] is True and v["persisted"] == .37, "Volume must show 0..100% and persist")
    require(e["speed_options"] == [.5, 1, 1.25, 1.5, 2] and all(abs(float(k) - r) < .001 for k, r in e["speed"].items()), "All five speeds must reach the media")
    k = e["keys"]
    require(k["space_plays"] is True and k["space_pauses"] is True and k["right_delta"] > 0 and k["left_delta"] < 0 and abs(k["up_delta"] - .05) < .011 and abs(k["down_delta"]) < .011
            and k["muted"] is True and k["unmuted"] is True and k["input_keeps_keys"] is True, "Keyboard shortcuts must act only outside inputs")
    require(all(e["fullscreen"][name] is True for name in ("on", "off", "esc_off")), "Native fullscreen must enter, exit and restore through Esc")
    require(e["subtitles"]["shown"] == "showing" and e["subtitles"]["off"] is True, "Subtitles must select and turn off")
    require(all(e["stream"][name] is True for name in ("session_changed", "same_resolution", "selected")), "Line change must open a new Session in the same Resolution")
    require(all(e["hide"][name] is True for name in ("concealed", "restored", "restored_by_focus")), "Controls must hide after 3s and return on movement or keyboard focus")
    require(abs(e["resume"]["position"] - 1.3) < .11 and e["resume"]["paused"] is True, "Resume must restore position once metadata arrived")
    require(e["seek_saved"] is True, "A seek must write a history snapshot")
    c = e["select_episode"]
    require(c["panelOpen"] is True and c["panelClosed"] is True and c["current"] and c["video"] == c["expected"] and c["opens"] == 1 and "Episode 3" in c["title"], "Episode selection must open exactly the chosen video")
    require(c["old_saved"] is True and c["old_position"] >= 1400 and c["new_first_position"] < 1000, "Switching must save the old episode without writing its progress to the new one")
    o = e["auto_off"]
    require(o["video"] == o["expected"] and o["matches"] == 0 and o["completed"] is True, "EOS with auto_next off must stay and record completion")
    require(e["stop"]["matches"] == 0 and e["stop"]["context"] is None, "A manual stop must not advance")
    n = e["auto_next"]
    require(n["video"] == n["expected"] and n["playing"] is True and n["session_bound"] is True and "Episode 4" in n["title"], "EOS with auto_next on must really play the next video")
    l = e["last"]
    require(l["next_disabled"] is True and l["matches"] == 0 and l["video"] == l["expected"], "The last episode must disable next and stay at EOS")
    a = e["ambiguous"]
    require(a["dialog"] is True and a["opens"] == 0 and a["session"] is None, "An ambiguous next video must wait for confirmation with nothing playing")
    saved = e["history"]["saved"]
    require(len(saved) == 1 and saved[0]["episode_total"] == 4 and saved[0]["source_id"] and saved[0]["title"], "History must keep one identified entry per title")
    require(e["errors"] == [], "Playback raised page errors: " + str(e["errors"]))


def webview(run):
    os.environ["T12_MIN_OPENS"] = "1"
    try:
        return t14.packaged(run, t12.LIVE_TEST, "t16", validate_playback, "playback", lambda e: dict(next=e["auto_next"]["video"]), bridge_class=Bridge, prepare_media=t12.owned_media)
    finally:
        os.environ.pop("T12_MIN_OPENS", None)


def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "--test-reporter=tap", "tests/playback-ui.test.ts", "tests/playback.test.ts", "tests/history.test.ts"], timeout=180)
    count, fail = re.search(r"# pass (\d+)", output), re.search(r"# fail (\d+)", output)
    require("REQ-T16" in output and count and int(count[1]) > 0 and fail and int(fail[1]) == 0, "Frontend tests must run nonzero with no failures")
    return dict(tests=int(count[1]))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT / "evidence/acceptance/T16-all.json" and args.junit.resolve() == ROOT / "evidence/acceptance/T16-all.xml", "Exact T16 output paths required")
    run = Acceptance(args.report, args.junit)
    if run.case("resource-ownership", ["REQ-T16"], "scope/source bound 4096MiB RSS sampled backend", lambda: resources(run, card="T0.16")):
        def oracles():
            output = run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_t16.py"])
            count = re.search(r"Ran (\d+) tests?", output)
            require(count and int(count[1]) >= 4, "Independent T16 oracle negatives must run")
            return dict(tests=int(count[1]))
        run.case("independent-negative-oracles", ["REQ-T16"], "corrupted playback UI evidence is rejected", oracles)
        run.case("frontend-behaviors", ["R23", "R25", "R32", "R42", "U09", "REQ-T16"], "nonzero player, history and next-video tests", lambda: frontend(run))
        run.case("packaged-playback-ui", IDS, "real Host and media: controls, keys, fullscreen, subtitles, lines, hide, resume, episode selection, EOS rules, history", lambda: webview(run))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
