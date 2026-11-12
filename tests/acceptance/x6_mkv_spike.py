"""T0.43: MKV feasibility spike with ffmpeg-next (experiments/mkv-remux).

Builds the spike against the trimmed static FFmpeg (route B, build_ffmpeg.py) and
reads the bundled full build (route A, --features bundled), then remuxes two MKV
samples and plays the output in a real WKWebView.
"""
import argparse
import json
import os
import re
import subprocess
import time
from pathlib import Path

import t14
from t00 import require

ROOT = Path(__file__).resolve().parents[2]
SPIKE = ROOT / "experiments/mkv-remux"
IDS = ["REQ-X07"]
PORT = 18743


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.43", "X6", IDS
    DEPLOYMENT = "experiment crate only; FFmpeg 9.0 built from source and linked statically; real WKWebView playback"
    DEFERRED = [dict(target="product integration", status="NOT RUN", reason="T0.44 after ADR and user decisions Q2/Q3")]


def binary(mode):
    return SPIKE / f"target/{mode}/release/mkv-remux-spike"


def build(run, mode):
    env = dict(CARGO_TARGET_DIR=str(SPIKE / f"target/{mode}"), CARGO_BUILD_JOBS="2")
    features = ["--features", "static"] if mode == "static" else ["--features", "bundled"]
    if mode == "static":
        env["FFMPEG_DIR"] = str(SPIKE / "target/ffmpeg-min")
        env["PKG_CONFIG_PATH"] = str(SPIKE / "target/ffmpeg-min/lib/pkgconfig")
    previous = {key: os.environ.get(key) for key in env}
    os.environ.update(env)
    try:
        started = time.monotonic()
        run.command(["cargo", "build", "--release", "--offline", "--manifest-path", str(SPIKE / "Cargo.toml"), *features], timeout=3600)
        return round(time.monotonic() - started, 1)
    finally:
        for key, value in previous.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value


def linked(path):
    output = subprocess.run(["otool", "-L", str(path)], capture_output=True, text=True, check=True).stdout
    libraries = [line.split()[0] for line in output.splitlines()[1:] if line.strip()]
    require(not any("libav" in lib or "libsw" in lib or "homebrew" in lib for lib in libraries), f"{path.name} still links a dynamic FFmpeg: {libraries}")
    return libraries


def static_build(run):
    seconds = build(run, "static")
    libraries = linked(binary("static"))
    ffmpeg = json.loads((SPIKE / "target/ffmpeg-min.json").read_text())
    run.observed["static"] = dict(build_seconds=seconds, ffmpeg_build_seconds=ffmpeg["seconds"], libraries=libraries,
                                  static_libs=ffmpeg["static_libs"], commit=ffmpeg["commit"])
    return run.observed["static"]


def size_report(run):
    require(binary("bundled").exists(), "route A (bundled) binary must have been built")
    linked(binary("bundled"))
    sizes = {mode: binary(mode).stat().st_size for mode in ("static", "bundled")}
    empty = SPIKE / "target/empty"
    if not empty.exists():
        subprocess.run(["rustc", "-O", "-o", str(empty), "-"], input="fn main() {}", text=True, check=True)
    sizes["empty_rust"] = empty.stat().st_size
    sizes["static_increase"] = sizes["static"] - sizes["empty_rust"]
    sizes["bundled_increase"] = sizes["bundled"] - sizes["empty_rust"]
    require(sizes["static_increase"] < sizes["bundled_increase"], "the trimmed build must be smaller than the full build")
    run.observed["sizes"] = sizes
    return sizes


def samples(run):
    folder = SPIKE / "samples"
    folder.mkdir(exist_ok=True)
    if not (folder / "h264_aac.mkv").exists() or not (folder / "hevc_dts.mkv").exists():
        # System ffmpeg only generates fixtures; the spike never links it.
        (folder / "sub.srt").write_text("1\n00:00:01,000 --> 00:00:04,000\nHello subtitle\n\n2\n00:00:05,000 --> 00:00:08,000\n第二句字幕\n")
        run.command(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=1280x720:rate=25:duration=12",
                     "-f", "lavfi", "-i", "sine=frequency=440:duration=12", "-i", str(folder / "sub.srt"), "-map", "0", "-map", "1", "-map", "2",
                     "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p", "-c:a", "aac", "-c:s", "srt", str(folder / "h264_aac.mkv")])
        run.command(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=1280x720:rate=25:duration=12",
                     "-f", "lavfi", "-i", "sine=frequency=330:duration=12", "-ac", "2", "-map", "0", "-map", "1", "-c:v", "libx265", "-preset", "veryfast",
                     "-pix_fmt", "yuv420p", "-x265-params", "log-level=error", "-c:a", "dca", "-strict", "-2", "-ar", "48000", str(folder / "hevc_dts.mkv")])


def remux(run, name, expected):
    samples(run)
    out = SPIKE / "out" / name
    output = run.command([str(binary("static")), str(SPIKE / f"samples/{name}.mkv"), str(out)], timeout=300)
    row = json.loads(next(line for line in output.splitlines() if line.startswith("REMUX:"))[6:])
    probe = run.command(["ffprobe", "-v", "error", "-show_entries", "stream=codec_name,codec_tag_string,channels", "-of", "compact", str(out / "index.m3u8")])
    for token in expected:
        require(token in probe, f"{name}: {token} missing from remuxed output")
    if not (SPIKE / "target/wkcheck").exists():
        run.command(["swiftc", "-O", str(SPIKE / "wkwebview_check.swift"), "-o", str(SPIKE / "target/wkcheck")], timeout=600)
    server = subprocess.Popen([str(ROOT / ".venv/bin/python"), "-I", "-m", "http.server", str(PORT), "--bind", "127.0.0.1", "--directory", str(SPIKE / "out")],
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        time.sleep(1)
        played = run.command([str(SPIKE / "target/wkcheck"), f"http://127.0.0.1:{PORT}/{name}/index.m3u8"], timeout=60)
    finally:
        server.terminate()
        server.wait(timeout=10)
    check = json.loads(next(line for line in played.splitlines() if line.startswith("WKCHECK:"))[8:])
    require(check["t"] > 3 and check["err"] is None and check["w"] == 1280, f"{name}: WKWebView playback failed {check}")
    run.observed[name] = dict(remux=row, wkwebview=check)
    return run.observed[name]


def license_check(run):
    ffmpeg = json.loads((SPIKE / "target/ffmpeg-min.json").read_text())
    require(not any(flag in ffmpeg["configure"] for flag in ("--enable-gpl", "--enable-nonfree", "--enable-version3")), "trimmed build must stay LGPL 2.1+")
    route_b = re.search(r'#define FFMPEG_LICENSE "([^"]+)"', (SPIKE / "target/ffmpeg-src/config.h").read_text())[1]
    configs = list((SPIKE / "target/bundled").rglob("ffmpeg-*/config.h"))
    require(configs, "route A FFmpeg config.h missing")
    route_a = re.search(r'#define FFMPEG_LICENSE "([^"]+)"', configs[0].read_text())[1]
    require("LGPL" in route_b and "LGPL" in route_a, f"licenses differ from LGPL: {route_b} / {route_a}")
    run.observed["license"] = dict(route_b=route_b, route_a=route_a)
    return run.observed["license"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    run.case("static-build", ["REQ-X07"], "spike links the trimmed FFmpeg statically; no libav/homebrew dylib", lambda: static_build(run))
    run.case("size-report", ["REQ-X07"], "binary size of trimmed (B) and full bundled (A) builds against an empty Rust binary", lambda: size_report(run))
    run.case("remux-h264-aac", ["REQ-X07"], "H.264+AAC+SRT copied to fMP4 HLS, SRT extracted, plays >3s in real WKWebView",
             lambda: remux(run, "h264_aac", ["codec_name=h264|codec_tag_string=avc1", "codec_name=aac"]))
    run.case("remux-hevc-dts", ["REQ-X07"], "HEVC tagged hvc1, DTS re-encoded to AAC, plays >3s in real WKWebView",
             lambda: remux(run, "hevc_dts", ["codec_name=hevc|codec_tag_string=hvc1", "codec_name=aac"]))
    run.case("license", ["REQ-X07"], "both routes configure FFmpeg as LGPL 2.1+ (no gpl/nonfree/version3)", lambda: license_check(run))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
