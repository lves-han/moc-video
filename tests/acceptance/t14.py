"""T0.14 home page acceptance: production App/HomeView against a real Host in a packaged WKWebView."""
import argparse
import datetime as dt
import json
import platform
import re
import sys
import tempfile
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path

from t00 import require, workflow, write_json
from t10 import resources
from t11 import Acceptance as BaseAcceptance
import t01
import t12

ROOT = Path(__file__).resolve().parents[2]
IDS = ["U03", "R01", "R21", "R34", "R35", "R37", "REQ-T14"]
LIVE_TEST = "host::maccms::tests::t08::t09::t14::t14_host_home_webview_bridge"


def validate_home(e):
    require(1 <= e["skeleton"] <= 24, "First load must show a limited skeleton")
    require(e["restored"] == dict(metadata="stremio", playback="maccms"), "Metadata and playback sources must restore independently")
    order = e["partial"]["order"]
    require([row["index"] for row in order] == [0, 1, 2] and [row["error"] for row in order] == [False, True, False]
            and order[1]["cards"] == 0 and order[0]["cards"] and order[2]["cards"] and e["partial"]["full_page_error"] is False,
            "A failed section must stay in place without replacing its peers")
    require(all(page["kept"] == page["before"] and page["after"] > page["before"] and page["requests"] == 1 for page in e["pages"]),
            "Pagination must keep cards during its tail load and append")
    p = e["pagination"]
    # Page 2 carries 105 metas with five repeats, so it must add exactly 100 cards.
    require(e["pages"][0]["after"] - e["pages"][0]["before"] == 100 and p["cards"] == 305 and p["unique"] == 305 and p["peers_unchanged"] is True,
            "Hundreds of cards must load deduplicated without touching peers")
    i = e["images"]
    require(i["lazy"] is True and i["posters"] > 0 and i["unique_urls"] == i["posters"] and i["placeholders"] > 0, "Posters must be lazy with a no-image placeholder")
    long = e["long_title"]
    require(long["title_chars"] > 100 and long["width"] <= long["card_width"] + 1 and long["height"] < 60, "Long titles must stay inside the card")
    layouts = e["layouts"]
    require(len(layouts) == 4 and not any(row["overflow"] for row in layouts), "Home must not overflow horizontally")
    # 1200 and 960 are the desktop default and minimum window; narrower widths only must not overflow.
    for row in layouts[:2]:
        require(4 <= row["columns"] <= 7 and 149 <= row["poster_width"] <= 181 and abs(row["ratio"] - 1.5) < .02,
                f"Desktop grid must show 4..7 cards of 150..180px at 2:3: {row}")
    sweep = e["column_sweep"]
    require(len(sweep) >= 8 and min(row["content_width"] for row in sweep) >= 640 and max(row["content_width"] for row in sweep) >= 1400, "Column sweep must span desktop content widths")
    for row in sweep:
        require(4 <= row["columns"] <= 7 and 149 <= row["poster_width"] <= 181, f"Every desktop width must show 4..7 cards of 150..180px: {row}")
    r = e["retry"]
    require(r["recovered"] > 0 and r["still_second"] is True and r["featured_kept"] is True and r["requests"] == 1 and r["error_cleared"] is True,
            "Section retry must recover only that section in place")
    back = e["return"]
    require(back["scrolled"] > 0 and abs(back["restored"] - back["scrolled"]) < 8 and back["cards_kept"] is True and back["requests"] == 0,
            "Return must restore scroll and loaded sections without refetching")
    s = e["playback_source"]
    require(s["kept_home"] is True and s["requests"] == 0 and s["persisted"] is True, "Playback source change must keep the metadata home")
    x = e["expiry"]
    require(x["status_queries"] == 1 and x["home_requests"] == 1 and x["page_one_snapshot"] is True and x["cards_after"] > 0 and x["error"] is False,
            "Expired poster must query once and refresh page one once")
    require(e["empty"]["retry"] is True and e["empty"]["selector"] is True and "no content" in e["empty"]["text"], "Empty source state differs")
    require(e["failed"]["retry"] is True and e["failed"]["selector"] is True and e["recovered"] > 0, "Failed source state must offer retry and source switching")
    require(e["errors"] == [], "Home raised page errors: " + json.dumps(e["errors"]))


def webview(run):
    return packaged(run, LIVE_TEST, "t14", validate_home, "home", lambda e: dict(cards=e["pagination"]["cards"], layouts=e["layouts"]))


def packaged(run, live_test, op, validate, key, summary, bridge_class=None, prepare_media=None):
    require(t01.native_gui_session()["screen_locked"] is not True, "macOS desktop locked; unlock before packaged WKWebView acceptance")
    previous = t01.DATA, t01.BUILD, t12.LIVE_TEST
    t01.DATA = run.folder / "packaged"
    t01.DATA.mkdir()
    t01.BUILD = t01.DATA / "build.json"
    t12.LIVE_TEST = live_test
    session, bridge = None, None
    try:
        t01.check_media()
        t01.build(run)
        receipt = t01.check_build(run.source)
        executable = ROOT / receipt["automation"]["path"] / "Contents/MacOS/yingxiang-video"
        with tempfile.TemporaryDirectory(prefix="t14-live-", dir=run.folder) as temporary:
            folder = Path(temporary)
            media = folder / "media"
            media.mkdir()
            try:
                if prepare_media:
                    prepare_media(run, media)
                bridge = (bridge_class or Bridge)(run, folder, media)
                ready = bridge.wait(folder / "ready.json", timeout=180)
                with t01.Fixture(t01.MEDIA, invoke=bridge.invoke) as fixture:
                    session = t01.NativeSession(run, fixture, executable, 1)
                    session.wait(0, 30)
                    # Registers WebKit helpers in the owned budget and cleanup, before and after their first use.
                    session.command("identity")
                    evidence = session.command(op, timeout=240, payload=ready)
                    session.record["commands"][-1]["args"] = dict(payload="actual Host bridge payload; capability URLs redacted")
                    run.observed[key] = evidence
                    validate(evidence)
                    session.command("identity")
                    require(session.record.get("webkit_pids", {}).get("renderer", 0) > 0, "WebKit renderer was not registered")
                    session.close()
                    require(session.record.get("exit") == 0 and session.record.get("lingering") == [] and session.record.get("peak_combined_rss_bytes", 0) > 0,
                            "Native session must exit cleanly with WebKit in its RSS budget")
                    require(fixture.active == 0, "Owned fixture HTTP remains")
                bridge.finish()
                return dict(summary(evidence), native="actual Host invoke/gateway and packaged WKWebView")
            finally:
                try:
                    if session is not None and not session.closed:
                        session.close(graceful=False)
                finally:
                    if bridge is not None:
                        bridge.close()
    finally:
        t01.DATA, t01.BUILD, t12.LIVE_TEST = previous


class Bridge(t12.Bridge):
    def finish(self):
        self.invoke(dict(operation="finish"))
        self.process.wait(timeout=30)
        self.reader.join(timeout=5)
        self.record["exit"] = self.process.returncode
        require(self.process.returncode == 0 and not self.reader.is_alive(), "Real Host bridge did not finish cleanly")
        vectors = [json.loads(line.split("T14_VECTOR:", 1)[1]) for line in self.lines if "T14_VECTOR:" in line]
        require(len(vectors) == 1 and vectors[0]["get_home"] > 0 and vectors[0]["active_requests"] == 0, "Real Host home evidence missing")
        require("test result: ok. 1 passed; 0 failed; 0 ignored;" in "".join(self.lines), "Actual named native test did not pass")
        require(not self.run.observed.get("host_log_sanitized"), "Native output contained credentials; stored log was sanitized")
        self.run.observed["host"] = vectors[0]


class Acceptance(BaseAcceptance):
    DEPLOYMENT = "packaged Tauri WKWebView with real Host/gateway and a controlled Stremio addon; no cluster or Android"
    CARD, ORIGINAL, REQUIREMENTS = "T0.14", "T14", IDS

    def __init__(self, report, junit):
        self.report_path, self.junit_path = report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / (f"{self.ORIGINAL}-{uuid.uuid4().hex}")
        self.folder.mkdir(parents=True)
        self.source = workflow.snapshot()
        self.cases, self.commands, self.observed = [], [], {}

    def save(self):
        current = workflow.snapshot()
        self.case("stable-tested-source", [f"REQ-{self.ORIGINAL}"], "unchanged source throughout A1", lambda: require(current == self.source, "source changed during A1"))
        summaries = []
        for identifier in self.REQUIREMENTS:
            children = [case for case in self.cases if identifier in case["requirements"]]
            summaries.append(dict(id=identifier, result="PASS" if children and all(case["result"] == "PASS" for case in children) else "FAIL", children=[case["id"] for case in children]))
        counts = dict(required=len(self.cases), ran=len(self.cases), passed=sum(case["result"] == "PASS" for case in self.cases),
                      failed=sum(case["result"] != "PASS" for case in self.cases), skipped=0)
        write_json(self.report_path, dict(card=self.CARD, original_card=self.ORIGINAL, suite="all", utc_started=self.started,
            utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(), source_before=self.source, tested_source_digest=current,
            protected=workflow.protected_hash(), platform=sys.platform, arch=platform.machine(), os=platform.platform(),
            observed=self.observed, commands=self.commands, cases=self.cases, summaries=summaries, counts=counts,
            deployment=self.DEPLOYMENT,
            deferred=[dict(target="real public Stremio addons/other platforms", status="NOT RUN", reason="T0.24 product end-to-end; ADR-005/006 local macOS arm64")]))
        rows = self.cases + summaries
        xml = ET.Element("testsuite", name=f"{self.ORIGINAL}-all", tests=str(len(rows)), errors="0", skipped="0",
                         failures=str(sum(row["result"] != "PASS" for row in rows)), timestamp=self.started)
        for row in rows:
            case = ET.SubElement(xml, "testcase", classname=self.ORIGINAL, name=row["id"])
            if row["result"] != "PASS":
                ET.SubElement(case, "failure", message=f"{self.ORIGINAL} behavior assertion failed").text = json.dumps(row, ensure_ascii=False)[:20000]
        self.junit_path.parent.mkdir(parents=True, exist_ok=True)
        ET.ElementTree(xml).write(self.junit_path, encoding="utf-8", xml_declaration=True)
        print(json.dumps(counts))
        return 0 if counts["failed"] == 0 and summaries and all(row["result"] == "PASS" for row in summaries) else 1


def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "--test-reporter=tap", "tests/home.test.ts", "tests/state.test.ts"], timeout=180)
    require("REQ-T14" in output, "Home image recovery tests did not run")
    count = re.search(r"# pass (\d+)", output)
    fail = re.search(r"# fail (\d+)", output)
    require(count and int(count[1]) > 0 and fail and int(fail[1]) == 0, "Frontend tests must run nonzero with no failures")
    return dict(tests=int(count[1]))


def main():
    if not __debug__:
        raise SystemExit("Acceptance requires assertions; remove -O")
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT / "evidence/acceptance/T14-all.json" and args.junit.resolve() == ROOT / "evidence/acceptance/T14-all.xml", "Exact T14 output paths required")
    run = Acceptance(args.report, args.junit)
    owned = run.case("resource-ownership", ["REQ-T14"], "scope/source bound 4096MiB RSS sampled backend", lambda: resources(run, card="T0.14"))
    if owned:
        def oracles():
            output = run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_t14.py"])
            count = re.search(r"Ran (\d+) tests?", output)
            require(count and int(count[1]) >= 4, "Independent T14 oracle negatives must run")
            return dict(tests=int(count[1]))
        run.case("independent-negative-oracles", ["REQ-T14"], "corrupted home evidence is rejected", oracles)
        run.case("frontend-behaviors", ["R34", "R35", "R37", "REQ-T14"], "complete nonzero frontend tests including home image recovery", lambda: frontend(run))
        run.case("packaged-home", IDS, "real Stremio catalog via invoke: skeleton, partial failure in place, pagination/dedupe, layout, return scroll, source independence, image expiry, empty/failed states", lambda: webview(run))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
