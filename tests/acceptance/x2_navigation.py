"""T0.39: sidebar Local video (always) and Live (when an enabled live list exists)."""
import argparse
import json
import re
from pathlib import Path

import t14
from t00 import require

ROOT = Path(__file__).resolve().parents[2]
IDS = ["REQ-X01", "REQ-X02"]


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.39", "X2", IDS
    DEPLOYMENT = "current frontend state test, Host settings validation and recorded browser check of the production sidebar"
    DEFERRED = [dict(target="packaged WKWebView sidebar", status="NOT RUN", reason="ADR-062 necessary checks; UI observed in browser over a fake host")]


def manual():
    return json.loads((ROOT / "evidence/manual/T0.39-navigation-ui.json").read_text())["observed"]


def local_always(run):
    app = (ROOT / "src/App.vue").read_text()
    require('{ page: "local", href: "#/local"' in app and '<LocalView v-else-if="route.page === \'local\'" />' in app, "Local video entry or page missing")
    ui = manual()
    require("#/local" in ui["desktop_nav_without_live"] and "#/local" in ui["desktop_nav_with_enabled_live"] and ui["local_page"]["highlighted_in_sidebar"], "Local video must always show")
    run.command(["npx", "vue-tsc", "--noEmit"], timeout=600)
    return dict(desktop=ui["desktop_nav_without_live"])


def live_conditional(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "--test-reporter=tap", "tests/x2.test.ts"], timeout=180)
    require(re.search(r"^# pass 1$", output, re.M) and re.search(r"^# fail 0$", output, re.M), "frontend live condition test must pass")
    native = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "--", "--test-threads=1", "settings::"], timeout=1800)
    require(re.search(r"test result: ok\. [3-9] passed; 0 failed", native), "Host settings validation for live lists must pass")
    run.command(["{python}", "-B", "scripts/check-contracts.py"], timeout=300)
    ui = manual()
    require("#/live" not in ui["desktop_nav_without_live"] and "#/live" in ui["desktop_nav_with_enabled_live"] and "#/live" in ui["mobile_nav_with_enabled_live"]
            and ui["live_hidden_after_disable"] and ui["live_hidden_after_delete"] and ui["page_errors"] == 0, "Live visibility differs")
    return dict(live=ui["live_page"])


def focus():
    order = manual()["arrow_down_focus_order"]
    require(order.index("#/local") < order.index("#/live") < order.index("#/history"), "focus must reach Local video and Live in menu order")
    return order


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    run.case("local-always", ["REQ-X01"], "Local video entry always present on desktop and mobile; page reachable", lambda: local_always(run))
    run.case("live-conditional", ["REQ-X02"], "Live appears only with an enabled live list; disable/delete hide it; schema and Host validation", lambda: live_conditional(run))
    run.case("focus", ["REQ-X02", "REQ-X01"], "keyboard focus reaches the new entries in order", focus)
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
