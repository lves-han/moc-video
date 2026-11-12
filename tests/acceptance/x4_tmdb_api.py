"""T0.41: TMDB official API mode on the real Host and current frontend."""
import argparse
import json
from pathlib import Path

import t14
from t00 import require

ROOT = Path(__file__).resolve().parents[2]
IDS = ["REQ-X06"]


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.41", "X4", IDS
    DEPLOYMENT = "real Rust Host, shared Stremio path and loopback gateway against a controlled TMDB API fixture; recorded browser check of TMDB settings"
    DEFERRED = [dict(target="real api.themoviedb.org", status="NOT RUN", reason="no API key available; fixture follows TMDB v3 response shapes")]


def native(run):
    output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib",
                          "--", "--test-threads=1", "--nocapture", "x4_tmdb_api_catalog_details_identity_endpoints_and_secrets", "x1_builtin_tmdb_language_instance_and_identity"], timeout=1800)
    require("test result: ok. 2 passed; 0 failed" in output, "TMDB API Host test and Stremio-mode regression must pass")
    rows = [json.loads(line.split("X4_TMDB_API:", 1)[1]) for line in output.splitlines() if line.startswith("X4_TMDB_API:")]
    require(len(rows) == 1 and rows[0]["same_source"] and rows[0]["episodes"] == 2 and rows[0]["error"] == "UNAUTHORIZED"
            and {"Inception", "Breaking Bad"} <= set(rows[0]["titles"]), "Host observation differs")
    run.observed["host"] = rows[0]
    return rows[0]


def checks(run):
    run.command(["{python}", "-B", "scripts/check-contracts.py"], timeout=300)
    run.command(["npx", "vue-tsc", "--noEmit"], timeout=600)
    ui = json.loads((ROOT / "evidence/manual/T0.41-tmdb-api-ui.json").read_text())["observed"]
    require(ui["choose_api_shows_form"]["key_input_type"] == "password" and ui["saved_api_settings"]["api_url"] == "https://proxy.test/3"
            and ui["switch_back_to_stremio"]["mode"] == "stremio" and ui["page_errors"] == 0, "recorded settings check differs")
    return ui["saved_api_settings"]["mode"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    host = {}
    def host_case():
        host.update(native(run)); return host
    run.case("api-catalog", ["REQ-X06"], "home, details (directors, cast roles, episodes) and search through the API mode on the real Host", host_case)
    run.case("shared-identity", ["REQ-X06"], "same provider id as Stremio mode; tmdb:<id> refs; series videos <imdb>:<s>:<e>", lambda: require(host.get("same_source"), "identity differs") or "same")
    run.case("custom-endpoint", ["REQ-X06"], "API and image addresses are used (fixture requests and gateway poster); settings UI saves custom addresses", lambda: [require(host, "Host case must pass"), checks(run)])
    run.case("invalid-key", ["REQ-X06"], "a rejected key reports UNAUTHORIZED with a clear message; v4 token uses the Authorization header", lambda: require(host.get("error") == "UNAUTHORIZED", "key error differs") or host["error"])
    run.case("secret-redaction", ["REQ-X06"], "v3 key, v4 token and wrong key never appear in source, home, details, search, sync or error responses (asserted in the Host test)", lambda: require(host, "Host case must pass") or "redacted")
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
