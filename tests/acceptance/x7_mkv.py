"""T0.44: local MKV playback through the real Host/gateway with the bundled LGPL FFmpeg dylibs."""
import argparse
import json
import os
import re
import shutil
import subprocess
from pathlib import Path

import t14
from t00 import require

ROOT = Path(__file__).resolve().parents[2]
IDS = ["REQ-X07"]
DIST = ROOT / "src-tauri/ffmpeg/dist"
TESTS = ["remux::tests", "x7_local_mkv_remux_seek_audio_subtitles_cleanup", "x7_large_mkv_seek_past_four_gigabytes", "x5_local_library_range_scope_history_and_restart"]


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.44", "X7", IDS
    DEPLOYMENT = "real Rust Host and gateway, trimmed LGPL FFmpeg 9.0 dylibs, real WKWebView playback; controlled MKV fixtures and a generated >4 GiB file"
    DEFERRED = [dict(target="network-source MKV", status="NOT RUN", reason="ADR-067 local files only")]


def large_file(run, folder):
    folder.mkdir(parents=True, exist_ok=True)
    clip, listing, large = folder / "clip.mkv", folder / "list.txt", folder / "lib/large.mkv"
    large.parent.mkdir(exist_ok=True)
    # System ffmpeg only generates the fixture: 60 s at a constant 40 Mbit/s, repeated past 4 GiB.
    run.command(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=1280x720:rate=25:duration=60",
                 "-f", "lavfi", "-i", "sine=frequency=440:duration=60", "-c:v", "libx264", "-preset", "ultrafast", "-g", "50",
                 "-b:v", "40M", "-minrate", "40M", "-maxrate", "40M", "-bufsize", "40M", "-x264-params", "nal-hrd=cbr:filler=1",
                 "-c:a", "aac", "-b:a", "128k", str(clip)], timeout=600)
    listing.write_text("".join(f"file '{clip}'\n" for _ in range(15)))
    run.command(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i", str(listing), "-c", "copy", str(large)], timeout=900)
    clip.unlink()
    require(large.stat().st_size > 4 * 1024 ** 3, "generated MKV must exceed 4 GiB")
    return large


def host(run):
    build = json.loads((DIST / "ffmpeg-build.json").read_text())
    require(build["license"].startswith("LGPL") and len(build["dylibs"]) == 4, "FFmpeg dylibs must be the LGPL build")
    tool = ROOT / "target/x7/wkcheck"
    tool.parent.mkdir(parents=True, exist_ok=True)
    run.command(["swiftc", "-O", "tests/fixtures/x7_wkcheck.swift", "-o", str(tool)], timeout=600)
    folder = ROOT / "target/x7/large"
    shutil.rmtree(folder, ignore_errors=True)
    try:
        large = large_file(run, folder)
        os.environ.update(X7_WKCHECK=str(tool), X7_LARGE=str(large))
        output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "--",
                              "--test-threads=1", "--nocapture", *TESTS], timeout=2400)
    finally:
        os.environ.pop("X7_WKCHECK", None)
        os.environ.pop("X7_LARGE", None)
        shutil.rmtree(folder, ignore_errors=True)
    require(f"test result: ok. {len(TESTS)} passed; 0 failed" in output, "MKV Host, large-file, remux and local tests must pass")
    rows = {key: json.loads(line.split(":", 1)[1]) for line in output.splitlines() for key in ("X7_MKV", "X7_LARGE") if line.startswith(key + ":")}
    require(set(rows) == {"X7_MKV", "X7_LARGE"}, "both observations required")
    require(len(rows["X7_MKV"]["wkwebview"]) == 2 and rows["X7_LARGE"]["wkwebview"], "real WKWebView playback must have run")
    run.observed.update(rows)
    return rows


def notices():
    inventory = json.loads((ROOT / "licenses/dependencies.json").read_text())
    names = {p["name"]: p["license"] for p in inventory["packages"]}
    require(names.get("FFmpeg") == "LGPL-2.1-or-later" and {"ffmpeg-next", "ffmpeg-sys-next", "tauri-plugin-dialog", "rfd"} <= set(names), "license inventory incomplete")
    text = (ROOT / "licenses/THIRD_PARTY_NOTICES.txt").read_text()
    require("bundled-binary: FFmpeg" in text and "GNU LESSER GENERAL PUBLIC LICENSE" in text, "notices lack the FFmpeg LGPL text")
    config = json.loads((ROOT / "src-tauri/tauri.conf.json").read_text())
    frameworks = config["bundle"]["macOS"]["frameworks"]
    require(len(frameworks) == 4, "four FFmpeg dylibs ship in Frameworks")
    for path in frameworks:
        library = ROOT / "src-tauri" / path
        name = subprocess.run(["otool", "-D", str(library)], capture_output=True, text=True, check=True).stdout.splitlines()[-1]
        require(name == f"@rpath/{library.name}" and str(Path.home()).encode() not in library.read_bytes(), f"{library.name} install name or contents differ")
    require("Contents/Frameworks" in (ROOT / "packaging/release.py").read_text(), "release payload check must allow the dylibs")
    bundle = json.loads((ROOT / "evidence/manual/T0.44-bundle-ui.json").read_text())["bundle"]
    require(bundle["frameworks_identical_to_build"] and bundle["release_rpaths"] == ["@executable_path/../Frameworks"], "recorded bundle check differs")
    return dict(packages=len(names), frameworks=frameworks)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    rows = {}
    def host_case():
        rows.update(host(run)); return rows["X7_LARGE"]
    run.case("large-file-seek", ["REQ-X07"], ">4 GiB MKV: segment past 4 GiB generated on demand with correct start; WKWebView seeks to 95% and plays", host_case)
    run.case("audio-transcode", ["REQ-X07"], "HEVC+DTS: DTS re-encoded to AAC, plays in WKWebView with an audio track", lambda: require(rows and rows["X7_MKV"]["wkwebview"][1]["audio"] == 1, "audio track missing") or rows["X7_MKV"]["wkwebview"][1])
    run.case("embedded-subtitles", ["REQ-X07"], "embedded SRT and ASS tracks served as WebVTT; Local video loads them as blob: text tracks",
             lambda: require(rows and rows["X7_MKV"]["subtitle"] == "chi" and json.loads((ROOT / "evidence/manual/T0.44-bundle-ui.json").read_text())["ui"]["track"]["cues"] == 4, "subtitles differ") or "vtt")
    run.case("cleanup", ["REQ-X07"], "closing stops serving; no MKV file handle or temporary segment output remains (asserted in the Host test)", lambda: require(rows, "Host case must pass") or "released")
    run.case("unsupported-codec", ["REQ-X07"], "MPEG-4 Part 2 video in MKV is refused with MEDIA_UNSUPPORTED (asserted in the Host test)", lambda: require(rows, "Host case must pass") or "refused")
    run.case("notices", ["REQ-X07"], "FFmpeg LGPL and new crates in the license inventory; @rpath dylibs without local paths in Frameworks",
             lambda: [notices(), run.command(["npx", "vue-tsc", "--noEmit"], timeout=600)][0])
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
