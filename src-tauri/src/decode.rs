//! Decodes an audio file into per-channel f32 sample buffers using `symphonia`.
//!
//! `symphonia` is the sole decoder for analysis purposes (see AGENTS.md "Décisions
//! actées") — `claxon` is deliberately not used here.

use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::tags::{self, EncoderTagMatch};

pub struct DecodedAudio {
    pub sample_rate: u32,
    pub channels: usize,
    pub codec_short_name: String,
    pub container_short_name: String,
    pub bits_per_sample: Option<u32>,
    /// One buffer per channel, full track length, samples normalized to [-1.0, 1.0].
    pub channel_samples: Vec<Vec<f32>>,
    /// `Some(true)`/`Some(false)`: the decoder checked the file's own embedded checksum
    /// (e.g. FLAC's STREAMINFO MD5) against what was actually decoded. `None`: this
    /// codec/container has no such embedded checksum to check (MP3, AAC, WAV, ...) — not
    /// an error, just nothing to verify.
    pub integrity_verified: Option<bool>,
    /// Container tag values matching a known lossy-encoder signature — see tags.rs
    /// module docs on why this is weaker evidence than the spectral indicators.
    pub encoder_tag_matches: Vec<EncoderTagMatch>,
}

pub fn decode_file(path: &Path) -> Result<DecodedAudio, String> {
    let file = File::open(path).map_err(|e| format!("cannot open file: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| format!("unsupported or corrupt file: {e}"))?;

    let container_short_name = format.format_info().short_name.to_string();
    let encoder_tag_matches = tags::scan_for_lossy_encoder_traces(&mut format);

    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| "no decodable audio track found".to_string())?
        .clone();

    let audio_params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| "track has no audio codec parameters".to_string())?
        .clone();

    // `verify(true)`: ask the decoder to check the file's own embedded checksum (e.g.
    // FLAC's STREAMINFO MD5 of the decoded PCM) if it has one — see
    // `decoder.finalize()` below. Free correctness signal; symphonia computes it as part
    // of normal decoding when supported, no separate pass needed.
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&audio_params, &AudioDecoderOptions::default().verify(true))
        .map_err(|e| format!("unsupported codec: {e}"))?;

    let codec_short_name = decoder.codec_info().short_name.to_string();
    let bits_per_sample = audio_params.bits_per_sample;
    let track_id = track.id;

    let mut sample_rate: u32 = 0;
    let mut channels: usize = 0;
    let mut channel_samples: Vec<Vec<f32>> = Vec::new();
    let mut interleaved_buf: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            // Advanced feature (chained OGG streams etc.) — treat as end of what we can
            // decode rather than restarting the whole pipeline.
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(format!("demux error: {e}")),
        };

        if packet.track_id != track_id {
            continue;
        }

        let audio_buf = match decoder.decode(&packet) {
            Ok(buf) => buf,
            Err(SymphoniaError::IoError(_)) | Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode error: {e}")),
        };

        if channel_samples.is_empty() {
            channels = audio_buf.spec().channels().count();
            if channels == 0 {
                return Err("decoded audio reports zero channels".to_string());
            }
            sample_rate = audio_buf.spec().rate();
            channel_samples = vec![Vec::new(); channels];
        }

        interleaved_buf.resize(audio_buf.samples_interleaved(), 0.0f32);
        audio_buf.copy_to_slice_interleaved(&mut interleaved_buf);

        for (i, sample) in interleaved_buf.iter().enumerate() {
            channel_samples[i % channels].push(*sample);
        }
    }

    if sample_rate == 0 || channel_samples.iter().all(|c| c.is_empty()) {
        return Err("no audio frames could be decoded from this file".to_string());
    }

    let integrity_verified = decoder.finalize().verify_ok;

    Ok(DecodedAudio {
        sample_rate,
        channels,
        codec_short_name,
        container_short_name,
        bits_per_sample,
        channel_samples,
        integrity_verified,
        encoder_tag_matches,
    })
}
