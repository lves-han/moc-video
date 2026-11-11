"""Prepare the platform's LGPL FFmpeg 9 shared libraries.

Run with the project interpreter: .venv/bin/python -I src-tauri/ffmpeg/build_ffmpeg.py
POSIX builds the trimmed pinned source; Windows verifies a pinned x64 shared SDK.
Cargo finds dist through FFMPEG_DIR in .cargo/config.toml.
"""
import hashlib
import json
import platform
import shutil
import subprocess
import sys
import time
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCE, PREFIX = HERE / "src", HERE / "dist"
BRANCH = "release/9.0"
COMMIT = "c179411bbb2864cc71323d3055acda25d5c2aace"
FAKE_PREFIX = "/opt/moc-ffmpeg"
# Only what local MKV playback needs: demux MKV, mux fragmented MP4, parse common codecs,
# probe H.264/HEVC (B-frame delay, never decoding pictures), decode non-WebKit audio and
# re-encode it to AAC. LGPL 2.1+: no --enable-gpl/version3/nonfree.
CONFIGURE = [
    "--disable-everything", "--disable-programs", "--disable-doc", "--disable-network", "--disable-autodetect",
    "--enable-shared", "--disable-static", "--enable-pic",
    "--disable-avdevice", "--disable-avfilter", "--disable-swscale",
    "--enable-avcodec", "--enable-avformat", "--enable-swresample",
    "--enable-protocol=file",
    "--enable-demuxer=matroska",
    "--enable-muxer=mp4,mov",
    "--enable-parser=h264,hevc,aac,ac3,dca,flac,opus,mpegaudio,vorbis",
    "--enable-bsf=h264_mp4toannexb,hevc_mp4toannexb,aac_adtstoasc,extract_extradata",
    "--enable-decoder=h264,hevc,dca,truehd,mlp,vorbis,ac3,eac3,flac,opus,aac,mp3,mp2,pcm_s16le,pcm_s16be,pcm_s24le,pcm_s32le,pcm_f32le,subrip,ass,ssa",
    "--enable-encoder=aac",
]

# Windows uses MSVC import libraries and LGPL shared DLLs, without a Unix shell.
WINDOWS_ASSET = "ffmpeg-n9.0.2-48-gd4070a14ce-win64-lgpl-shared-9.0.zip"
WINDOWS_URL = "https://github.com/FFmpeg-Builds/FFmpeg-Builds/releases/download/autobuild-2026-11-10-12-39/" + WINDOWS_ASSET
WINDOWS_SHA256 = "cdc19c20aa80d46f78cfc69c8d87430ee34990864e30379db02d1447cd111e83"


def windows_sdk():
    if platform.machine().lower() not in {"amd64", "x86_64"}:
        raise SystemExit("The pinned Windows FFmpeg SDK requires x64")
    archive = HERE.parent.parent / ".cache" / "ffmpeg" / WINDOWS_ASSET
    archive.parent.mkdir(parents=True, exist_ok=True)
    if not archive.exists():
        run(["curl.exe", "--fail", "--location", "--output", str(archive), WINDOWS_URL], HERE)
    with archive.open("rb") as stream:
        if hashlib.file_digest(stream, "sha256").hexdigest() != WINDOWS_SHA256:
            raise SystemExit(f"FFmpeg SDK SHA256 mismatch: {archive}")
    with zipfile.ZipFile(archive) as bundle:
        for member in bundle.infolist():
            parts = Path(member.filename).parts[1:]
            if not parts or member.is_dir():
                continue
            target = PREFIX.joinpath(*parts).resolve()
            if not target.is_relative_to(PREFIX.resolve()):
                raise SystemExit("FFmpeg SDK archive path escapes dist")
            target.parent.mkdir(parents=True, exist_ok=True)
            with bundle.open(member) as source, target.open("wb") as output:
                shutil.copyfileobj(source, output)
    report = dict(source=WINDOWS_URL, sha256=WINDOWS_SHA256, license="LGPL version 3 or later")
    (PREFIX / "windows-sdk.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    print(f"Add {PREFIX / 'bin'} to PATH before invoking Cargo; libclang is also required.")


def run(argv, cwd):
    print("+", " ".join(argv), flush=True)
    subprocess.run(argv, cwd=cwd, check=True)


def main():
    if sys.platform == "win32":
        return windows_sdk()
    started = time.monotonic()
    if not (SOURCE / "configure").exists():
        run(["git", "clone", "--no-checkout", "https://github.com/FFmpeg/FFmpeg", str(SOURCE)], HERE)
        run(["git", "checkout", "--detach", COMMIT], SOURCE)
    commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=SOURCE, capture_output=True, text=True, check=True).stdout.strip()
    if commit != COMMIT:
        raise SystemExit(f"FFmpeg source must be at {COMMIT}; existing checkout is {commit}")
    # FFmpeg embeds its configure line in the libraries; a fixed prefix keeps local paths out.
    configure = [*CONFIGURE]
    if sys.platform == "darwin":
        configure.append("--install-name-dir=@rpath")
    run(["./configure", f"--prefix={FAKE_PREFIX}", *configure], SOURCE)
    run(["make", "-j2"], SOURCE)
    stage = HERE / "stage"
    shutil.rmtree(stage, ignore_errors=True)
    shutil.rmtree(PREFIX, ignore_errors=True)
    run(["make", "install", f"DESTDIR={stage}"], SOURCE)
    shutil.move(str(stage / FAKE_PREFIX.lstrip("/")), PREFIX)
    shutil.rmtree(stage)
    pattern = "*.dylib" if sys.platform == "darwin" else "*.so*"
    for library in (PREFIX / "lib").glob(pattern):
        require_clean = library.read_bytes()
        if str(Path.home()).encode() in require_clean:
            raise SystemExit(f"local path inside {library.name}")
    libs = sorted(p.name for p in (PREFIX / "lib").glob(pattern) if not p.is_symlink())
    license_line = next(line for line in (SOURCE / "config.h").read_text().splitlines() if "FFMPEG_LICENSE" in line)
    report = dict(source="https://github.com/FFmpeg/FFmpeg", branch=BRANCH, commit=commit, configure=configure,
                  license=license_line.split('"')[1], dylibs=libs, seconds=round(time.monotonic() - started))
    (PREFIX / "ffmpeg-build.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    sys.exit(main())
