//! Local MKV playback (ADR-066/067): a VOD fMP4 HLS playlist cut at MKV keyframes.
//! Opening reads the keyframe index (Cues); each segment request seeks to its own
//! keyframe and muxes only that span, so seeking never reprocesses earlier media.
//! Video and WebKit-decodable audio are copied; other audio is re-encoded to AAC.
use crate::contracts::ApiError;
use ff::{Rational, codec, format, media};
use ffmpeg_next as ff;
use std::{
    fs::File,
    io::{Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        Arc, Once,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

const TARGET_SECONDS: f64 = 4.0;
const COPY_AUDIO: [codec::Id; 6] = [
    codec::Id::AAC,
    codec::Id::MP3,
    codec::Id::AC3,
    codec::Id::EAC3,
    codec::Id::FLAC,
    codec::Id::OPUS,
];
const AAC_FRAME: usize = 1024;
static INIT: Once = Once::new();
static TEMP: AtomicU64 = AtomicU64::new(0);

fn unsupported(message: &str) -> ApiError {
    ApiError::new("MEDIA_UNSUPPORTED", message)
}

fn failed(stage: &str) -> impl Fn(ff::Error) -> ApiError + '_ {
    move |error| ApiError::new("INTERNAL_ERROR", &format!("MKV {stage} failed: {error}"))
}

#[cfg(unix)]
fn open(path: &Path) -> Result<format::context::Input, ApiError> {
    INIT.call_once(|| {
        let _ = ff::init();
        ff::log::set_level(ff::log::Level::Fatal);
    });
    format::input(path).map_err(|_| {
        ApiError::new(
            "SOURCE_NOT_FOUND",
            "Local file was moved, deleted or is not a readable MKV",
        )
    })
}

#[cfg(unix)]
fn open_cancellable(
    path: &Path,
    cancelled: &Arc<AtomicBool>,
) -> Result<format::context::Input, ApiError> {
    let token = cancelled.clone();
    format::input_with_interrupt(path, move || token.load(Ordering::SeqCst)).map_err(|_| {
        if cancelled.load(Ordering::SeqCst) {
            ApiError::cancelled()
        } else {
            ApiError::new(
                "SOURCE_NOT_FOUND",
                "Local file was moved, deleted or is not a readable MKV",
            )
        }
    })
}

#[cfg(unix)]
fn descriptor_path(file: &File) -> Result<PathBuf, ApiError> {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        Ok(PathBuf::from(format!("/dev/fd/{}", file.as_raw_fd())))
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        Err(unsupported(
            "Secure MKV playback is unavailable on this platform",
        ))
    }
}

fn open_held(file: &File, cancelled: &Arc<AtomicBool>) -> Result<format::context::Input, ApiError> {
    #[cfg(unix)]
    {
        open_cancellable(&descriptor_path(file)?, cancelled)
    }
    #[cfg(windows)]
    {
        INIT.call_once(|| {
            let _ = ff::init();
            ff::log::set_level(ff::log::Level::Fatal);
        });
        let stream = file
            .try_clone()
            .map_err(|_| ApiError::new("SOURCE_NOT_FOUND", "Cannot retain local file"))?;
        let io =
            format::context::StreamIo::from_read_seek(stream).map_err(failed("file reader"))?;
        let token = cancelled.clone();
        // The wrapper owns the authorized handle; no path is reopened by FFmpeg.
        format::input_from_stream_with_interrupt(io, None, None, move || {
            token.load(Ordering::SeqCst)
        })
        .map_err(|error| {
            if cancelled.load(Ordering::SeqCst) {
                ApiError::cancelled()
            } else {
                failed("file open")(error)
            }
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (file, cancelled);
        Err(unsupported(
            "Secure MKV playback is unavailable on this platform",
        ))
    }
}

#[derive(Clone, Debug)]
pub struct SubtitleTrack {
    pub index: usize,
    pub label: String,
    pub language: Option<String>,
    pub ass: bool,
}

#[derive(Clone, Debug)]
struct Audio {
    index: usize,
    copy: bool,
    rate: u32,
}

#[derive(Debug)]
pub struct Plan {
    pub path: PathBuf,
    source: Arc<File>,
    pub duration: f64,
    /// Segment spans in seconds, each starting at a video keyframe.
    pub segments: Vec<(f64, f64)>,
    pub subtitles: Vec<SubtitleTrack>,
    video: usize,
    audio: Option<Audio>,
    init: Vec<u8>,
}

/// Top-level MP4 boxes, so init (ftyp+moov) and media (moof+mdat) can be separated.
fn boxes(data: &[u8]) -> Vec<(&[u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= data.len() {
        let size = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        let (size, header) = if size == 1 && at + 16 <= data.len() {
            (
                u64::from_be_bytes(data[at + 8..at + 16].try_into().unwrap()) as usize,
                16,
            )
        } else {
            (size, 8)
        };
        if size < header || at + size > data.len() {
            break;
        }
        out.push((
            data[at + 4..at + 8].try_into().unwrap(),
            &data[at..at + size],
        ));
        at += size;
    }
    out
}

struct Encoder {
    decoder: codec::decoder::Audio,
    encoder: codec::encoder::Audio,
    // Created from the first decoded frame, whose layout may need normalizing.
    resampler: Option<ff::software::resampling::Context>,
    pending: Vec<Vec<f32>>,
    next: i64,
}

/// Output streams mirror the input choices; init and segments share these parameters.
fn add_streams(
    plan: &Plan,
    input: &format::context::Input,
    output: &mut format::context::Output,
) -> Result<Option<Encoder>, ApiError> {
    let video = input
        .stream(plan.video)
        .ok_or_else(|| unsupported("MKV video stream missing"))?;
    let mut ost = output
        .add_stream(ff::encoder::find(codec::Id::None))
        .map_err(failed("video stream"))?;
    ost.set_parameters(video.parameters());
    // Apple players require the hvc1 sample entry for HEVC.
    let tag = if video.parameters().id() == codec::Id::HEVC {
        u32::from_le_bytes(*b"hvc1")
    } else {
        0
    };
    unsafe { (*ost.parameters().as_mut_ptr()).codec_tag = tag };
    let Some(audio) = &plan.audio else {
        return Ok(None);
    };
    let stream = input
        .stream(audio.index)
        .ok_or_else(|| unsupported("MKV audio stream missing"))?;
    if audio.copy {
        let mut ost = output
            .add_stream(ff::encoder::find(codec::Id::None))
            .map_err(failed("audio stream"))?;
        ost.set_parameters(stream.parameters());
        unsafe { (*ost.parameters().as_mut_ptr()).codec_tag = 0 };
        return Ok(None);
    }
    let decoder = codec::context::Context::from_parameters(stream.parameters())
        .and_then(|c| c.decoder().audio())
        .map_err(|_| unsupported("This audio format cannot be decoded"))?;
    let aac =
        ff::encoder::find(codec::Id::AAC).ok_or_else(|| unsupported("AAC encoder unavailable"))?;
    // PCM tracks often carry an unspecified-order layout, which AAC rejects; use the standard one.
    let channels = decoder.channel_layout().channels().clamp(1, 8);
    let layout = ff::ChannelLayout::default(if decoder.channel_layout().channels() > 0 {
        channels
    } else {
        2
    });
    let mut ost = output.add_stream(aac).map_err(failed("AAC stream"))?;
    let mut encoder = codec::context::Context::from_parameters(ost.parameters())
        .and_then(|c| c.encoder().audio())
        .map_err(failed("AAC setup"))?;
    encoder.set_rate(audio.rate as i32);
    encoder.set_channel_layout(layout);
    encoder.set_format(format::Sample::F32(format::sample::Type::Planar));
    encoder.set_bit_rate(96_000 * layout.channels().max(2) as usize);
    encoder.set_time_base((1, audio.rate as i32));
    encoder.set_flags(codec::flag::Flags::GLOBAL_HEADER);
    ost.set_time_base((1, audio.rate as i32));
    let encoder = encoder.open_as(aac).map_err(failed("AAC open"))?;
    ost.set_parameters(&encoder);
    Ok(Some(Encoder {
        decoder,
        encoder,
        resampler: None,
        pending: vec![Vec::new(); layout.channels() as usize],
        next: 0,
    }))
}

fn temp_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "moc-remux-{}-{}.mp4",
        std::process::id(),
        TEMP.fetch_add(1, Ordering::Relaxed)
    ))
}

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn movflags() -> ff::Dictionary<'static> {
    let mut options = ff::Dictionary::new();
    // frag_discont keeps each segment's tfdt at its own media time.
    options.set(
        "movflags",
        "frag_keyframe+empty_moov+default_base_moof+frag_discont+delay_moov",
    );
    options
}

impl Plan {
    pub fn open(path: &Path) -> Result<Plan, ApiError> {
        let file = File::open(path)
            .map_err(|_| ApiError::new("SOURCE_NOT_FOUND", "Local file was moved or deleted"))?;
        Self::open_file(path, file)
    }

    pub fn open_file(path: &Path, mut file: File) -> Result<Plan, ApiError> {
        file.seek(SeekFrom::Start(0))
            .map_err(|_| ApiError::internal("MKV seek failed"))?;
        #[cfg(unix)]
        let mut input = open(&descriptor_path(&file)?)?;
        #[cfg(not(unix))]
        let mut input = open_held(&file, &Arc::new(AtomicBool::new(false)))?;
        let video = input
            .streams()
            .best(media::Type::Video)
            .ok_or_else(|| unsupported("This MKV has no video"))?;
        let (video_index, video_id, video_tb) =
            (video.index(), video.parameters().id(), video.time_base());
        if !matches!(video_id, codec::Id::H264 | codec::Id::HEVC) {
            return Err(unsupported("Only H.264 and HEVC video can be played"));
        }
        let audio = input.streams().best(media::Type::Audio).map(|s| {
            let params = s.parameters();
            let rate = codec::context::Context::from_parameters(params.clone())
                .ok()
                .and_then(|c| c.decoder().audio().ok())
                .map(|d| d.rate())
                .unwrap_or(48_000);
            Audio {
                index: s.index(),
                copy: COPY_AUDIO.contains(&params.id()),
                rate,
            }
        });
        let subtitles = input
            .streams()
            .filter(|s| {
                matches!(
                    s.parameters().id(),
                    codec::Id::SUBRIP | codec::Id::ASS | codec::Id::SSA
                )
            })
            .map(|s| {
                let language = s
                    .metadata()
                    .get("language")
                    .filter(|l| *l != "und")
                    .map(str::to_owned);
                let title = s.metadata().get("title").map(str::to_owned);
                SubtitleTrack {
                    index: s.index(),
                    label: title
                        .or_else(|| language.clone())
                        .unwrap_or_else(|| format!("Subtitle {}", s.index())),
                    language,
                    ass: s.parameters().id() != codec::Id::SUBRIP,
                }
            })
            .collect();
        let duration = (input.duration().max(0) as f64) / f64::from(ff::ffi::AV_TIME_BASE);
        if duration <= 0.0 {
            return Err(unsupported("This MKV needs a known duration for playback"));
        }
        // Seeking makes the Matroska demuxer load its Cues into the stream index.
        let _ = input.seek(0, ..);
        let seconds = |ts: i64| ts as f64 * f64::from(video_tb.0) / f64::from(video_tb.1);
        let mut keyframes: Vec<f64> = unsafe {
            let stream = input.stream(video_index).unwrap().as_ptr() as *mut ff::ffi::AVStream;
            (0..ff::ffi::avformat_index_get_entries_count(stream))
                .filter_map(|i| ff::ffi::avformat_index_get_entry(stream, i).as_ref())
                .filter(|e| e.flags() & ff::ffi::AVINDEX_KEYFRAME != 0)
                .map(|e| seconds(e.timestamp))
                .collect()
        };
        if keyframes.len() < 2 && duration > TARGET_SECONDS {
            return Err(unsupported(
                "This MKV needs a keyframe index (Cues) for playback",
            ));
        }
        keyframes.sort_by(f64::total_cmp);
        keyframes.dedup();
        let first = keyframes.first().copied().unwrap_or(0.0);
        let end = duration.max(keyframes.last().copied().unwrap_or(0.0) + 0.001);
        let mut segments = Vec::new();
        let mut start = first;
        for &k in &keyframes[keyframes.len().min(1)..] {
            if k - start >= TARGET_SECONDS {
                segments.push((start, k));
                start = k;
            }
        }
        segments.push((start, end));
        let mut plan = Plan {
            path: path.to_owned(),
            source: Arc::new(file),
            duration: end,
            segments,
            subtitles,
            video: video_index,
            audio,
            init: Vec::new(),
        };
        plan.init = plan.make_init(&input)?;
        Ok(plan)
    }

    /// Open a separate file description for each request and compare its identity
    /// with the authorized one before FFmpeg can read from it.
    fn reopen(&self) -> Result<File, ApiError> {
        let missing = || {
            ApiError::new(
                "SOURCE_NOT_FOUND",
                "Local file was moved, deleted or replaced",
            )
        };
        let file = File::open(&self.path).map_err(|_| missing())?;
        #[cfg(windows)]
        if !crate::host::local::same_file(&self.source, &file)? {
            return Err(missing());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let old = self.source.metadata().map_err(|_| missing())?;
            let new = file.metadata().map_err(|_| missing())?;
            if old.dev() != new.dev() || old.ino() != new.ino() {
                return Err(missing());
            }
        }
        Ok(file)
    }

    fn make_init(&self, input: &format::context::Input) -> Result<Vec<u8>, ApiError> {
        let temp = Temp(temp_path());
        let mut output = format::output_as(&temp.0, "mp4").map_err(failed("init output"))?;
        add_streams(self, input, &mut output)?;
        output
            .write_header_with(movflags())
            .map_err(failed("init header"))?;
        output.write_trailer().map_err(failed("init trailer"))?;
        drop(output);
        let data =
            std::fs::read(&temp.0).map_err(|_| ApiError::internal("MKV init read failed"))?;
        Ok(boxes(&data)
            .into_iter()
            .filter(|(kind, _)| matches!(*kind, b"ftyp" | b"moov"))
            .flat_map(|(_, b)| b.to_vec())
            .collect())
    }

    pub fn init(&self) -> &[u8] {
        &self.init
    }

    pub fn playlist(&self, url: impl Fn(&str) -> String) -> String {
        let longest = self
            .segments
            .iter()
            .map(|(a, b)| b - a)
            .fold(1.0, f64::max)
            .ceil();
        let mut text = format!(
            "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:{longest}\n#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-INDEPENDENT-SEGMENTS\n#EXT-X-MAP:URI=\"{}\"\n",
            url("init")
        );
        for (n, (a, b)) in self.segments.iter().enumerate() {
            text.push_str(&format!(
                "#EXTINF:{:.3},\n{}\n",
                b - a,
                url(&format!("s{n}"))
            ));
        }
        text.push_str("#EXT-X-ENDLIST\n");
        text
    }

    /// One fragmented-MP4 media segment (moof+mdat), muxed from its own keyframe.
    pub fn segment(&self, n: usize) -> Result<Vec<u8>, ApiError> {
        self.segment_cancellable(n, &Arc::new(AtomicBool::new(false)))
    }

    pub fn segment_cancellable(
        &self,
        n: usize,
        cancelled: &Arc<AtomicBool>,
    ) -> Result<Vec<u8>, ApiError> {
        let (start, end) = *self
            .segments
            .get(n)
            .ok_or_else(|| ApiError::bad("Unknown MKV segment"))?;
        let last = n + 1 == self.segments.len();
        let mut held = self.reopen()?;
        if cancelled.load(Ordering::SeqCst) {
            return Err(ApiError::cancelled());
        }
        held.seek(SeekFrom::Start(0))
            .map_err(|_| ApiError::internal("MKV seek failed"))?;
        let mut input = open_held(&held, cancelled)?;
        let temp = Temp(temp_path());
        let mut output = format::output_as(&temp.0, "mp4").map_err(failed("segment output"))?;
        let mut encoder = add_streams(self, &input, &mut output)?;
        output
            .write_header_with(movflags())
            .map_err(failed("segment header"))?;
        let target = (start * f64::from(ff::ffi::AV_TIME_BASE)) as i64;
        input.seek(target, ..target + 1).map_err(failed("seek"))?;
        let time_bases: Vec<Rational> = input.streams().map(|s| s.time_base()).collect();
        let secs = |ts: i64, tb: Rational| ts as f64 * f64::from(tb.0) / f64::from(tb.1);
        let audio_out = 1;
        let mut video_done = false;
        let mut audio_done = self.audio.is_none();
        for (stream, mut packet) in input.packets() {
            if cancelled.load(Ordering::SeqCst) {
                return Err(ApiError::cancelled());
            }
            let index = stream.index();
            let tb = time_bases[index];
            let at = packet.pts().or(packet.dts()).map(|t| secs(t, tb));
            if index == self.video {
                if video_done {
                    continue;
                }
                if packet.is_key() && at.is_some_and(|t| !last && t >= end - 0.001) {
                    video_done = true;
                } else if at.is_none_or(|t| t >= start - 0.001) {
                    packet.rescale_ts(tb, output.stream(0).unwrap().time_base());
                    packet.set_position(-1);
                    packet.set_stream(0);
                    packet
                        .write_interleaved(&mut output)
                        .map_err(failed("video write"))?;
                }
            } else if self.audio.as_ref().is_some_and(|a| a.index == index) && !audio_done {
                let Some(t) = at else { continue };
                if !last && t >= end {
                    audio_done = true;
                } else if t >= start {
                    match encoder.as_mut() {
                        None => {
                            packet.rescale_ts(tb, output.stream(audio_out).unwrap().time_base());
                            packet.set_position(-1);
                            packet.set_stream(audio_out);
                            packet
                                .write_interleaved(&mut output)
                                .map_err(failed("audio write"))?;
                        }
                        Some(e) => {
                            if e.next == 0 && e.pending[0].is_empty() {
                                e.next = (t * f64::from(e.encoder.rate())).round() as i64;
                            }
                            e.decoder
                                .send_packet(&packet)
                                .map_err(failed("audio decode"))?;
                            transcode(e, &mut output, audio_out, false)?;
                        }
                    }
                }
            }
            if video_done && audio_done {
                break;
            }
        }
        if cancelled.load(Ordering::SeqCst) {
            return Err(ApiError::cancelled());
        }
        if let Some(e) = encoder.as_mut() {
            let _ = e.decoder.send_eof();
            transcode(e, &mut output, audio_out, true)?;
            let _ = e.encoder.send_eof();
            drain(e, &mut output, audio_out)?;
        }
        output.write_trailer().map_err(failed("segment trailer"))?;
        drop(output);
        let data =
            std::fs::read(&temp.0).map_err(|_| ApiError::internal("MKV segment read failed"))?;
        Ok(boxes(&data)
            .into_iter()
            .filter(|(kind, _)| matches!(*kind, b"moof" | b"mdat"))
            .flat_map(|(_, b)| b.to_vec())
            .collect())
    }

    /// One embedded text track as WebVTT, reading only that track's packets.
    pub fn subtitle(&self, index: usize) -> Result<String, ApiError> {
        self.subtitle_cancellable(index, &Arc::new(AtomicBool::new(false)))
    }

    pub fn subtitle_cancellable(
        &self,
        index: usize,
        cancelled: &Arc<AtomicBool>,
    ) -> Result<String, ApiError> {
        let track = self
            .subtitles
            .iter()
            .find(|t| t.index == index)
            .ok_or_else(|| ApiError::bad("Unknown subtitle track"))?;
        let mut held = self.reopen()?;
        if cancelled.load(Ordering::SeqCst) {
            return Err(ApiError::cancelled());
        }
        held.seek(SeekFrom::Start(0))
            .map_err(|_| ApiError::internal("MKV seek failed"))?;
        let mut input = open_held(&held, cancelled)?;
        unsafe {
            for i in 0..input.nb_streams() as usize {
                if i != index {
                    let stream = input.stream(i).unwrap().as_ptr() as *mut ff::ffi::AVStream;
                    (*stream).discard = ff::ffi::AVDiscard::AVDISCARD_ALL;
                }
            }
        }
        let tb = input.stream(index).unwrap().time_base();
        let ms = |ts: i64| ts * 1000 * i64::from(tb.0) / i64::from(tb.1);
        let mut cues = Vec::new();
        for (stream, packet) in input.packets() {
            if cancelled.load(Ordering::SeqCst) {
                return Err(ApiError::cancelled());
            }
            if stream.index() != index {
                continue;
            }
            let start = ms(packet.pts().unwrap_or(0));
            let raw = String::from_utf8_lossy(packet.data().unwrap_or(&[])).into_owned();
            // Matroska ASS events are "ReadOrder,Layer,Style,Name,MarginL,MarginR,MarginV,Effect,Text".
            let text = if track.ass {
                raw.splitn(9, ',').nth(8).unwrap_or("").to_owned()
            } else {
                raw
            };
            let text = strip_tags(&text);
            if !text.trim().is_empty() {
                cues.push((start, start + ms(packet.duration()).max(1), text));
            }
        }
        if cancelled.load(Ordering::SeqCst) {
            return Err(ApiError::cancelled());
        }
        cues.sort_by_key(|c| c.0);
        let stamp = |ms: i64| {
            format!(
                "{:02}:{:02}:{:02}.{:03}",
                ms / 3_600_000,
                ms / 60_000 % 60,
                ms / 1000 % 60,
                ms % 1000
            )
        };
        let mut vtt = String::from("WEBVTT\n\n");
        for (a, b, text) in cues {
            vtt.push_str(&format!(
                "{} --> {}\n{}\n\n",
                stamp(a),
                stamp(b),
                text.trim()
            ));
        }
        Ok(vtt)
    }
}

/// Plain cue text: ASS override blocks and HTML-like tags removed, markup characters escaped.
fn strip_tags(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in text.replace("\\N", "\n").replace("\\n", "\n").chars() {
        match c {
            '{' | '<' => depth += 1,
            '}' | '>' if depth > 0 => depth -= 1,
            _ if depth == 0 => match c {
                '&' => out.push_str("&amp;"),
                _ => out.push(c),
            },
            _ => {}
        }
    }
    out
}

fn transcode(
    e: &mut Encoder,
    output: &mut format::context::Output,
    stream: usize,
    flush: bool,
) -> Result<(), ApiError> {
    let mut frame = ff::frame::Audio::empty();
    while e.decoder.receive_frame(&mut frame).is_ok() {
        if frame.channel_layout().bits() == 0 {
            frame.set_channel_layout(ff::ChannelLayout::default(i32::from(frame.channels())));
        }
        if e.resampler.is_none() {
            e.resampler = Some(
                ff::software::resampling::Context::get(
                    frame.format(),
                    frame.channel_layout(),
                    frame.rate(),
                    format::Sample::F32(format::sample::Type::Planar),
                    e.encoder.channel_layout(),
                    e.encoder.rate(),
                )
                .map_err(failed("resampler"))?,
            );
        }
        let mut converted = ff::frame::Audio::empty();
        e.resampler
            .as_mut()
            .expect("Resampler")
            .run(&frame, &mut converted)
            .map_err(failed("resample"))?;
        for ch in 0..e.pending.len() {
            e.pending[ch].extend_from_slice(&converted.plane::<f32>(ch)[..converted.samples()]);
        }
    }
    while e.pending[0].len() >= AAC_FRAME || (flush && !e.pending[0].is_empty()) {
        let count = e.pending[0].len().min(AAC_FRAME);
        let mut frame = ff::frame::Audio::new(
            format::Sample::F32(format::sample::Type::Planar),
            count,
            e.encoder.channel_layout(),
        );
        frame.set_rate(e.encoder.rate());
        for ch in 0..e.pending.len() {
            let samples: Vec<f32> = e.pending[ch].drain(..count).collect();
            frame.plane_mut::<f32>(ch)[..count].copy_from_slice(&samples);
        }
        frame.set_pts(Some(e.next));
        e.next += count as i64;
        e.encoder.send_frame(&frame).map_err(failed("AAC encode"))?;
        drain(e, output, stream)?;
    }
    Ok(())
}

fn drain(
    e: &mut Encoder,
    output: &mut format::context::Output,
    stream: usize,
) -> Result<(), ApiError> {
    let mut packet = ff::Packet::empty();
    while e.encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(stream);
        packet.rescale_ts(
            Rational(1, e.encoder.rate() as i32),
            output.stream(stream).unwrap().time_base(),
        );
        packet
            .write_interleaved(output)
            .map_err(failed("AAC write"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/media")
            .join(name)
    }

    #[test]
    fn plan_segments_init_subtitles_and_unsupported() {
        let plan = Plan::open(&fixture("x7_h264_aac_srt.mkv")).unwrap();
        assert_eq!(plan.segments.len(), 3, "{:?}", plan.segments);
        assert!((plan.segments[1].0 - 4.0).abs() < 0.01 && (plan.duration - 12.0).abs() < 0.1);
        let kinds: Vec<_> = boxes(plan.init()).iter().map(|(k, _)| **k).collect();
        assert_eq!(kinds, vec![*b"ftyp", *b"moov"]);
        let segment = plan.segment(1).unwrap();
        let kinds: Vec<_> = boxes(&segment).iter().map(|(k, _)| **k).collect();
        assert!(
            kinds.first() == Some(b"moof") && kinds.contains(b"mdat"),
            "{kinds:?}"
        );
        assert_eq!(plan.segment(3).unwrap_err().code, "BAD_REQUEST");
        let cancelled = Arc::new(AtomicBool::new(true));
        assert_eq!(
            plan.segment_cancellable(0, &cancelled).unwrap_err().code,
            "CANCELLED"
        );
        assert_eq!(
            plan.subtitle_cancellable(plan.subtitles[0].index, &cancelled)
                .unwrap_err()
                .code,
            "CANCELLED"
        );
        let vtt = plan.subtitle(plan.subtitles[0].index).unwrap();
        assert!(
            vtt.starts_with("WEBVTT")
                && vtt.contains("00:00:01.000 --> 00:00:04.000\nHello subtitle")
                && vtt.contains("第二句字幕"),
            "{vtt}"
        );
        assert_eq!(plan.subtitles[0].language.as_deref(), Some("chi"));

        let hevc = Plan::open(&fixture("x7_hevc_dts_ass.mkv")).unwrap();
        let init = String::from_utf8_lossy(hevc.init()).into_owned();
        assert!(
            init.contains("hvc1") && init.contains("mp4a"),
            "HEVC tagged hvc1, DTS re-encoded to AAC"
        );
        assert!(!hevc.segment(2).unwrap().is_empty());
        assert!(
            hevc.subtitle(hevc.subtitles[0].index)
                .unwrap()
                .contains("Hello subtitle")
        );

        let pcm = Plan::open(&fixture("x7_h264_pcm.mkv")).unwrap();
        assert!(
            String::from_utf8_lossy(pcm.init()).contains("mp4a")
                && !pcm.segment(0).unwrap().is_empty(),
            "PCM audio re-encoded to AAC"
        );
        assert_eq!(
            Plan::open(&fixture("x7_mpeg4.mkv")).unwrap_err().code,
            "MEDIA_UNSUPPORTED"
        );
        let unindexed = Plan::open(&fixture("x7_noindex_live.mkv")).unwrap_err();
        assert_eq!(unindexed.code, "MEDIA_UNSUPPORTED");
        assert!(unindexed.message.contains("duration") || unindexed.message.contains("index"));
        assert_eq!(
            Plan::open(&fixture("missing.mkv")).unwrap_err().code,
            "SOURCE_NOT_FOUND"
        );
    }
}
