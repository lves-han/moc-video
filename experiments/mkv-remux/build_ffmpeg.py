"""T0.43 spike: build a trimmed, LGPL, static FFmpeg 9.0 for MKV -> fMP4 HLS remuxing.

Usage: python -I build_ffmpeg.py   (writes target/ffmpeg-src and target/ffmpeg-min)
"""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCE, PREFIX = HERE / "target/ffmpeg-src", HERE / "target/ffmpeg-min"
# Only what MKV playback needs: demux MKV/MP4/TS, mux fMP4 HLS, parse common codecs,
# and decode non-WebKit audio (DTS/TrueHD/Vorbis) for re-encoding to AAC.
CONFIGURE = [
    "--disable-everything", "--disable-programs", "--disable-doc", "--disable-network", "--disable-autodetect",
    "--enable-static", "--disable-shared", "--enable-pic",
    "--disable-avdevice", "--disable-avfilter", "--disable-swscale",
    "--enable-avcodec", "--enable-avformat", "--enable-swresample",
    "--enable-protocol=file",
    "--enable-demuxer=matroska,mov,mpegts",
    "--enable-muxer=hls,mp4,mov",
    "--enable-parser=h264,hevc,aac,ac3,dca,flac,opus,mpegaudio,vorbis",
    "--enable-bsf=h264_mp4toannexb,hevc_mp4toannexb,aac_adtstoasc,extract_extradata",
    # h264/hevc decoders are needed only while probing: they report B-frame delay so
    # copied packets get monotonic DTS. Pictures are never decoded.
    "--enable-decoder=h264,hevc,dca,truehd,mlp,vorbis,ac3,eac3,flac,opus,aac,mp3",
    "--enable-encoder=aac",
]


def run(argv, cwd):
    print("+", " ".join(argv), flush=True)
    subprocess.run(argv, cwd=cwd, check=True)


def main():
    started = time.monotonic()
    if not (SOURCE / "configure").exists():
        SOURCE.parent.mkdir(parents=True, exist_ok=True)
        run(["git", "clone", "--depth=1", "-b", "release/9.0", "https://github.com/FFmpeg/FFmpeg", str(SOURCE)], HERE)
    commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=SOURCE, capture_output=True, text=True, check=True).stdout.strip()
    run(["./configure", f"--prefix={PREFIX}", *CONFIGURE], SOURCE)
    run(["make", "-j2"], SOURCE)
    run(["make", "install"], SOURCE)
    libs = {p.name: p.stat().st_size for p in sorted((PREFIX / "lib").glob("*.a"))}
    report = dict(commit=commit, configure=CONFIGURE, seconds=round(time.monotonic() - started), static_libs=libs, jobs=2)
    (HERE / "target/ffmpeg-min.json").write_text(json.dumps(report, indent=2))
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    sys.exit(main())
