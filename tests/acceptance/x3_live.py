"""T0.40: live lists (IPTV M3U and live-source TXT) on the real Host/gateway and the current frontend."""
import argparse
import json
import re
from pathlib import Path

import t14
from t00 import require

ROOT = Path(__file__).resolve().parents[2]
IDS = ["REQ-X02"]


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.40", "X3", IDS
    DEPLOYMENT = "real Rust Host, live parser and loopback gateway with a controlled HTTP fixture; recorded browser check of the production Live page"
    DEFERRED = [dict(target="packaged WKWebView live HLS decoding", status="NOT RUN", reason="ADR-062 necessary checks; gateway output verified by Host test")]


def native(run):
    output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib",
                          "--", "--test-threads=1", "--nocapture", "host::live::", "x3_live_lists_gateway_refresh_cache_and_history"], timeout=1800)
    require("test result: ok. 4 passed; 0 failed" in output, "three parser tests and the Host live test must pass")
    rows = [json.loads(line.split("X3_LIVE:", 1)[1]) for line in output.splitlines() if line.startswith("X3_LIVE:")]
    require(rows == [dict(cache_used=0, formats=["m3u", "txt"], playlist_requests=2, segment_requests=2)], "Host live observation differs")
    run.observed["host"] = rows[0]
    return rows[0]


def contracts(run):
    run.command(["{python}", "-B", "scripts/check-contracts.py"], timeout=300)
    run.command(["npx", "vue-tsc", "--noEmit"], timeout=600)
    caps = json.loads((ROOT / "src-tauri/capabilities/main.json").read_text())["permissions"]
    require({"allow-get-live-channels", "allow-open-live"} <= set(caps), "live commands need their permissions")
    return "contracts, types and permissions"


def manual():
    return json.loads((ROOT / "evidence/manual/T0.40-live-ui.json").read_text())["observed"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    host = {}
    def host_case():
        host.update(native(run)); return host
    run.case("m3u", ["REQ-X02"], "M3U groups, logos, names; parser and real Host", host_case)
    run.case("txt", ["REQ-X02"], "TXT #genre# groups and alternatives detected from content", lambda: require(host.get("formats") == ["m3u", "txt"], "Host must detect both formats") or host["formats"])
    run.case("unsupported-protocol", ["REQ-X02"], "rtp/udp/rtmp channels listed as unsupported, disabled in UI and refused by open_live (MEDIA_UNSUPPORTED in Host test)",
             lambda: require(host and manual()["unsupported_channel_disabled"], "unsupported channel handling differs") or "refused")
    run.case("live-gateway", ["REQ-X02"], "each playlist request reaches upstream; segments bypass the media cache; no progress bar; sessions closed on switch/leave",
             lambda: [require(host.get("playlist_requests") == 2 and host.get("segment_requests") == 2 and host.get("cache_used") == 0, "gateway live mode differs"),
                      require(manual()["video_native_controls"] is False and manual()["leaving_page_closes_session"], "player UI differs"), contracts(run)])
    run.case("no-history", ["REQ-X02"], "live playback writes no watch history (Host list empty, no save_history from UI)",
             lambda: require(host and manual()["save_history_calls"] == 0, "history written") or "none")
    run.case("errors", ["REQ-X02"], "unrecognized list, missing config, stale revision, unknown index and unplayable stream report explicit errors",
             lambda: require(host and manual()["unplayable_stream_message"], "error reporting differs") or "explicit")
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
