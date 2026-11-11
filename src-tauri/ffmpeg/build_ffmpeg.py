"""Build the trimmed LGPL FFmpeg 9.0 as dynamic libraries for the app (ADR-066/067).

Run with the project interpreter: .venv/bin/python -I src-tauri/ffmpeg/build_ffmpeg.py
Writes src-tauri/ffmpeg/src (git checkout) and src-tauri/ffmpeg/dist (headers, @rpath dylibs).
Cargo finds dist through FFMPEG_DIR in .cargo/config.toml; the app bundle copies the dylibs into Contents/Frameworks.
"""
import json
import shutil
import subprocess
import sys
import time
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
    "--enable-shared", "--disable-static", "--enable-pic", "--install-name-dir=@rpath",
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


def run(argv, cwd):
    print("+", " ".join(argv), flush=True)
    subprocess.run(argv, cwd=cwd, check=True)


def main():
    started = time.monotonic()
    if not (SOURCE / "configure").exists():
        run(["git", "clone", "--no-checkout", "https://github.com/FFmpeg/FFmpeg", str(SOURCE)], HERE)
        run(["git", "checkout", "--detach", COMMIT], SOURCE)
    commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=SOURCE, capture_output=True, text=True, check=True).stdout.strip()
    if commit != COMMIT:
        raise SystemExit(f"FFmpeg source must be at {COMMIT}; existing checkout is {commit}")
    # FFmpeg embeds its configure line in the libraries; a fixed prefix keeps local paths out.
    run(["./configure", f"--prefix={FAKE_PREFIX}", *CONFIGURE], SOURCE)
    run(["make", "-j2"], SOURCE)
    stage = HERE / "stage"
    shutil.rmtree(stage, ignore_errors=True)
    shutil.rmtree(PREFIX, ignore_errors=True)
    run(["make", "install", f"DESTDIR={stage}"], SOURCE)
    shutil.move(str(stage / FAKE_PREFIX.lstrip("/")), PREFIX)
    shutil.rmtree(stage)
    for library in (PREFIX / "lib").glob("*.dylib"):
        require_clean = library.read_bytes()
        if str(Path.home()).encode() in require_clean:
            raise SystemExit(f"local path inside {library.name}")
    libs = sorted(p.name for p in (PREFIX / "lib").glob("*.dylib") if not p.is_symlink())
    license_line = next(line for line in (SOURCE / "config.h").read_text().splitlines() if "FFMPEG_LICENSE" in line)
    report = dict(source="https://github.com/FFmpeg/FFmpeg", branch=BRANCH, commit=commit, configure=CONFIGURE,
                  license=license_line.split('"')[1], dylibs=libs, seconds=round(time.monotonic() - started))
    (PREFIX / "ffmpeg-build.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    sys.exit(main())
