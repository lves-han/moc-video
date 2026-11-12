//! T0.43 spike: MKV -> fMP4 HLS for WKWebView.
//! Video and WebKit-decodable audio are copied; other audio (DTS, TrueHD, Vorbis)
//! is decoded and re-encoded to AAC. SRT subtitles are extracted to .srt text.
//! Usage: mkv-remux-spike <input.mkv> <output-dir>
use ffmpeg_next as ff;
use ff::{ChannelLayout, Rational, codec, format, media};
use std::{fmt::Write as _, path::Path, time::Instant};

const COPY_AUDIO: [codec::Id; 6] = [codec::Id::AAC, codec::Id::MP3, codec::Id::AC3, codec::Id::EAC3, codec::Id::FLAC, codec::Id::OPUS];
const AAC_FRAME: usize = 1024;

struct AudioTranscode {
    input: usize,
    output: usize,
    decoder: codec::decoder::Audio,
    encoder: codec::encoder::Audio,
    resampler: ff::software::resampling::Context,
    pending: Vec<Vec<f32>>,
    next_pts: i64,
    in_time_base: Rational,
}

impl AudioTranscode {
    fn drain(&mut self, octx: &mut format::context::Output, final_flush: bool) -> Result<(), ff::Error> {
        let channels = self.pending.len();
        while self.pending[0].len() >= AAC_FRAME || (final_flush && !self.pending[0].is_empty()) {
            let count = self.pending[0].len().min(AAC_FRAME);
            let mut frame = ff::frame::Audio::new(format::Sample::F32(format::sample::Type::Planar), count, self.encoder.channel_layout());
            frame.set_rate(self.encoder.rate());
            for ch in 0..channels {
                let samples: Vec<f32> = self.pending[ch].drain(..count).collect();
                frame.plane_mut::<f32>(ch)[..count].copy_from_slice(&samples);
            }
            frame.set_pts(Some(self.next_pts));
            self.next_pts += count as i64;
            self.encoder.send_frame(&frame)?;
            self.write(octx)?;
        }
        Ok(())
    }
    fn write(&mut self, octx: &mut format::context::Output) -> Result<(), ff::Error> {
        let mut packet = ff::Packet::empty();
        while self.encoder.receive_packet(&mut packet).is_ok() {
            packet.set_stream(self.output);
            packet.rescale_ts(Rational(1, self.encoder.rate() as i32), octx.stream(self.output).unwrap().time_base());
            packet.write_interleaved(octx)?;
        }
        Ok(())
    }
    fn push(&mut self, frame: &ff::frame::Audio) -> Result<(), ff::Error> {
        let mut converted = ff::frame::Audio::empty();
        self.resampler.run(frame, &mut converted)?;
        for ch in 0..self.pending.len() {
            self.pending[ch].extend_from_slice(&converted.plane::<f32>(ch)[..converted.samples()]);
        }
        Ok(())
    }
}

fn srt_time(ms: i64) -> String {
    format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let mut args = std::env::args().skip(1);
    let input = args.next().ok_or("missing input")?;
    let out = std::path::PathBuf::from(args.next().ok_or("missing output dir")?);
    std::fs::create_dir_all(&out)?;
    ff::init()?;
    ff::log::set_level(if std::env::var_os("SPIKE_DEBUG").is_some() { ff::log::Level::Verbose } else { ff::log::Level::Error });

    let mut ictx = format::input(&input)?;
    let mut octx = format::output_as(&out.join("index.m3u8"), "hls")?;
    let mut mapping: Vec<Option<usize>> = vec![None; ictx.nb_streams() as usize];
    let mut time_bases = vec![Rational(0, 1); ictx.nb_streams() as usize];
    let mut transcode: Option<AudioTranscode> = None;
    let mut subtitles: Vec<(usize, String, usize)> = Vec::new();
    let mut report = serde_like::Report::default();
    for ist in ictx.streams() {
        let params = ist.parameters();
        let index = ist.index();
        time_bases[index] = ist.time_base();
        match params.medium() {
            media::Type::Video if mapping.iter().all(|m| m.is_none()) || matches!(params.id(), codec::Id::H264 | codec::Id::HEVC) => {
                let mut ost = octx.add_stream(ff::encoder::find(codec::Id::None))?;
                ost.set_parameters(params.clone());
                // Apple players need the hvc1 sample entry for HEVC in MP4.
                let tag = if params.id() == codec::Id::HEVC { u32::from_le_bytes(*b"hvc1") } else { 0 };
                unsafe { (*ost.parameters().as_mut_ptr()).codec_tag = tag };
                mapping[index] = Some(ost.index());
                report.streams.push(format!("video {:?} copy", params.id()));
            }
            media::Type::Audio if transcode.is_none() && !mapping.iter().enumerate().any(|(i, m)| m.is_some() && i != index && ictx.stream(i).unwrap().parameters().medium() == media::Type::Audio) => {
                if COPY_AUDIO.contains(&params.id()) {
                    let mut ost = octx.add_stream(ff::encoder::find(codec::Id::None))?;
                    ost.set_parameters(params.clone());
                    unsafe { (*ost.parameters().as_mut_ptr()).codec_tag = 0 };
                    mapping[index] = Some(ost.index());
                    report.streams.push(format!("audio {:?} copy", params.id()));
                } else {
                    let decoder = codec::context::Context::from_parameters(params.clone())?.decoder().audio()?;
                    let aac = ff::encoder::find(codec::Id::AAC).ok_or("AAC encoder missing")?;
                    let layout = if decoder.channel_layout().channels() > 0 { decoder.channel_layout() } else { ChannelLayout::STEREO };
                    let global = octx.format().flags().contains(format::flag::Flags::GLOBAL_HEADER);
                    let mut ost = octx.add_stream(aac)?;
                    let mut encoder = codec::context::Context::from_parameters(ost.parameters())?.encoder().audio()?;
                    encoder.set_rate(decoder.rate() as i32);
                    encoder.set_channel_layout(layout);
                    encoder.set_format(format::Sample::F32(format::sample::Type::Planar));
                    encoder.set_bit_rate(192_000 * layout.channels().max(2) as usize / 2);
                    encoder.set_time_base((1, decoder.rate() as i32));
                    if global {
                        encoder.set_flags(codec::flag::Flags::GLOBAL_HEADER);
                    }
                    ost.set_time_base((1, decoder.rate() as i32));
                    let encoder = encoder.open_as(aac)?;
                    ost.set_parameters(&encoder);
                    let resampler = ff::software::resampling::Context::get(decoder.format(), decoder.channel_layout(), decoder.rate(),
                        format::Sample::F32(format::sample::Type::Planar), layout, decoder.rate())?;
                    report.streams.push(format!("audio {:?} -> AAC {}ch {}Hz", params.id(), layout.channels(), decoder.rate()));
                    let output = ost.index();
                    transcode = Some(AudioTranscode { input: index, output, decoder, encoder, resampler,
                        pending: vec![Vec::new(); layout.channels() as usize], next_pts: 0, in_time_base: ist.time_base() });
                }
            }
            media::Type::Subtitle if params.id() == codec::Id::SUBRIP => {
                subtitles.push((index, String::new(), 0));
                report.streams.push("subtitle SUBRIP -> srt".into());
            }
            _ => report.streams.push(format!("{:?} {:?} skipped", params.medium(), params.id())),
        }
    }
    let mut options = ff::Dictionary::new();
    options.set("hls_segment_type", "fmp4");
    options.set("hls_time", "4");
    options.set("hls_playlist_type", "vod");
    options.set("hls_flags", "independent_segments");
    options.set("hls_fmp4_init_filename", "init.mp4");
    options.set("hls_segment_filename", &out.join("seg%03d.m4s").to_string_lossy());
    octx.write_header_with(options)?;

    for (stream, mut packet) in ictx.packets() {
        let index = stream.index();
        if let Some(t) = transcode.as_mut().filter(|t| t.input == index) {
            t.decoder.send_packet(&packet)?;
            let mut frame = ff::frame::Audio::empty();
            while t.decoder.receive_frame(&mut frame).is_ok() {
                t.push(&frame)?;
            }
            t.drain(&mut octx, false)?;
            continue;
        }
        if let Some(sub) = subtitles.iter_mut().find(|s| s.0 == index) {
            let tb = time_bases[index];
            let ms = |v: i64| v * 1000 * tb.0 as i64 / tb.1 as i64;
            let start = ms(packet.pts().unwrap_or(0));
            let text = String::from_utf8_lossy(packet.data().unwrap_or(&[])).trim().to_owned();
            sub.2 += 1;
            let _ = write!(sub.1, "{}\n{} --> {}\n{}\n\n", sub.2, srt_time(start), srt_time(start + ms(packet.duration())), text);
            continue;
        }
        let Some(output) = mapping[index] else { continue };
        packet.rescale_ts(time_bases[index], octx.stream(output).unwrap().time_base());
        packet.set_position(-1);
        packet.set_stream(output);
        packet.write_interleaved(&mut octx)?;
    }
    if let Some(t) = transcode.as_mut() {
        t.decoder.send_eof()?;
        let mut frame = ff::frame::Audio::empty();
        while t.decoder.receive_frame(&mut frame).is_ok() {
            t.push(&frame)?;
        }
        t.drain(&mut octx, true)?;
        t.encoder.send_eof()?;
        t.write(&mut octx)?;
        let _ = t.in_time_base;
    }
    octx.write_trailer()?;
    for (index, text, count) in &subtitles {
        std::fs::write(out.join(format!("sub-{index}.srt")), text)?;
        report.subtitles.push(format!("stream {index}: {count} cues"));
    }
    report.seconds = started.elapsed().as_secs_f64();
    report.print(Path::new(&input));
    Ok(())
}

mod serde_like {
    /// Minimal JSON line for the acceptance script; avoids another dependency.
    #[derive(Default)]
    pub struct Report { pub streams: Vec<String>, pub subtitles: Vec<String>, pub seconds: f64 }
    impl Report {
        pub fn print(&self, input: &std::path::Path) {
            let list = |v: &[String]| v.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>().join(",");
            println!("REMUX:{{\"input\":{:?},\"streams\":[{}],\"subtitles\":[{}],\"seconds\":{:.3}}}",
                input.file_name().unwrap().to_string_lossy(), list(&self.streams), list(&self.subtitles), self.seconds);
        }
    }
}
