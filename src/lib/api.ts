import { invoke } from "@tauri-apps/api/core";



export interface ChannelStats {
  channel: number;
  peak_dbfs: number;
  rms_dbfs: number;
  crest_factor_db: number;
  /** Samples on the top quantization step of the declared depth — how often the signal
   * touched the rail, not evidence of clipping by itself. */
  full_scale_sample_count: number;
  /** Runs of 3+ consecutive full-scale samples: a flattened waveform top. This is the one
   * that means clipping. */
  clipped_run_count: number;
}

export interface SignalAnalysis {
  peak_dbfs: number;
  true_peak_dbtp: number;
  rms_dbfs: number;
  lufs_integrated: number | null;
  /** EBU Tech 3342 Loudness Range, in LU — companion metric to lufs_integrated. */
  loudness_range_lu: number | null;
  /** Oversampling factor ebur128 applied: 4 below 96 kHz, 2 up to 192, and 1 at 192 kHz and
   * above — where no oversampling happens and the figure is a sampled peak, not a true peak.
   * The UI must label it accordingly. */
  true_peak_oversampling: number;
  full_scale_sample_count_total: number;
  clipped_run_count_total: number;
  per_channel: ChannelStats[];
}

/** Whether the whole file reached the analysis. Mirrors `DecodeStatus` in decode.rs.
 *
 * Anything but `complete` means every measurement in the report describes a fragment, and
 * `transcode_assessment` withholds its verdict. */
export interface DecodeStatus {
  complete: boolean;
  skipped_packets: number;
  /** The stream asked to restart part-way through and decoding stopped there. */
  stopped_early: boolean;
  /** The channels came out different lengths, so different sections of the report describe
   * different amounts of audio. Only happens on a damaged or truncated stream. */
  channels_unequal: boolean;
}

export interface DynamicRangeResult {
  /** The "DR12"-style whole-file value from the Pleasurize Music Foundation algorithm,
   * rounded to an integer, averaged across channels. Distinct from crest_factor_db above. */
  dr14: number | null;
  per_channel_db: (number | null)[];
}

export interface FileInfo {
  filename: string;
  container: string;
  codec: string;
  sample_rate_hz: number;
  bit_depth: number | null;
  channels: number;
  duration_seconds: number;
  nyquist_hz: number;
  file_size_bytes: number;
  sample_count: number;
  bitrate_kbps: number | null;
  /** true/false: this file's own embedded checksum was checked (FLAC MD5). null: either
   * this codec has none (MP3/AAC/WAV) or this particular file didn't have one embedded
   * (common on re-tagged FLACs) — check `codec === "flac"` to tell those apart. */
  integrity_verified: boolean | null;
  decode_status: DecodeStatus;
}

export interface SpectrogramData {
  time_bin_count: number;
  frequency_bin_count: number;
  max_frequency_hz: number;
  duration_seconds: number;
  /** Row-major [time][frequency], u8 dB-mapped intensity, base64-encoded. */
  intensity_base64: string;
}

/** Steady-state level of one named frequency band, relative to the loudest band in the
 * same file (so always ≤ 0). Mirrors `BandLevel` in spectral.rs. */
export interface BandLevel {
  low_hz: number;
  /** null for the band that runs to Nyquist. */
  high_hz: number | null;
  level_db: number;
}

/** Side/mid energy for one band. Mirrors `BandStereo` in stereo.rs. */
export interface BandStereo {
  name: string;
  low_hz: number;
  high_hz: number | null;
  side_to_mid_db: number;
}

/** How the two channels relate. Mirrors `StereoAnalysis` in stereo.rs — reported
 * information only, deliberately not an input to the transcode verdict. */
export interface StereoAnalysis {
  /** -1..=1. 1 is identical channels, 0 unrelated, negative means out of phase. */
  correlation: number;
  side_to_mid_db: number;
  /** Channels are bit-identical: mono content in a stereo container. Exact, not a threshold. */
  dual_mono: boolean;
  effectively_mono: boolean;
  /** Negative correlation — summing to mono will cancel content, not just narrow it. */
  mono_compatibility_risk: boolean;
  per_band: BandStereo[];
}

export interface SpectralAnalysis {
  /** Where content stops. **null when nothing bounded it**, which is not the same as
   * "reaches Nyquist" and must never be rendered as a frequency. Raw measurement, not a
   * transcode verdict — see spectral.rs module docs. */
  spectral_cutoff_hz: number | null;
  rolloff_steepness_db_per_khz: number;
  /** Position of the codec-like edge, or null when no lowpass exists anywhere above 8 kHz
   * — the latter being the evidence behind a "probably authentic" verdict. */
  encoder_edge_hz: number | null;
  /** Same length/time alignment as spectrogram.time_bin_count. */
  cutoff_over_time_hz: number[];
  /** Standard deviation of cutoff_over_time_hz, in Hz. Reported, never scored: a mastering
   * lowpass is as fixed as a codec's, so stability does not separate them — see
   * spectral.rs. */
  cutoff_stability_hz: number;
  band_levels_db: BandLevel[];
  /** How far the stopband sits below the passband, in dB. null when no edge was found. */
  stopband_depth_db: number | null;
  /** Level of the top quarter of the declared band (never below the 22.05 kHz CD ceiling)
   * relative to the 1–22.05 kHz reference band, in dB. null at 44.1/48 kHz, where the
   * question does not arise. The only positive evidence of authenticity in the report. */
  above_cd_ceiling_db: number | null;
  spectrogram: SpectrogramData;
}

/** `declared_lossy` is not a verdict about deception: the file is in a lossy format and says
 * so, which means the question the other three answer — is a lossless container hiding lossy
 * audio — does not apply. The UI omits the confidence percentage for it. */
export type Verdict =
  | "probably_authentic"
  | "probably_transcoded"
  | "indeterminate"
  | "declared_lossy";

/** One stated observation behind a verdict, discriminated on `code`.
 *
 * Mirrors `IndicatorDetail` in transcode_detect.rs, which serializes the tag flattened
 * alongside `message` (see `Indicator` below). Frequencies arrive in kHz because that is
 * the unit every message quotes. Adding a variant here without adding it to the `indicator`
 * switch in i18n.svelte.ts is a type error, which is the point. */
export type IndicatorDetail =
  | {
      code: "encoder_tag_matched";
      tag_key: string;
      tag_value: string;
      matched_pattern: string;
      /** Further *tags* beyond the one quoted; 0 when it was the only one. Counted by
       * distinct (key, value), not by pattern hit. */
      additional_tags: number;
    }
  | { code: "tag_is_only_evidence" }
  | { code: "tag_contradicts_spectrum" }
  | { code: "invalid_sample_rate" }
  | { code: "sharp_rolloff"; steepness_db_per_khz: number; edge_khz: number }
  | { code: "no_encoder_lowpass"; scanned_from_khz: number; nyquist_khz: number }
  | { code: "transparent_encode_unseen" }
  | { code: "gradual_rolloff"; cutoff_khz: number; steepness_db_per_khz: number }
  | {
      code: "mdct_grid_aligned";
      z_score: number;
      frame_offset: number;
      zero_percent: number;
      baseline_percent: number;
    }
  | { code: "mdct_grid_clear" }
  | { code: "content_above_cd_ceiling"; level_db: number; ceiling_khz: number }
  | { code: "declared_lossy_codec"; codec: string }
  | { code: "decode_incomplete"; skipped_packets: number; stopped_early: boolean };

/** A piece of evidence, carrying the backend's English prose *and* the raw observation.
 *
 * `message` is what the CLI prints and what an exported report preserves, so a report reads
 * the same whatever language the UI was in. The UI renders it directly in English and
 * re-composes it from `code` + measurements in French — see `indicator` in i18n.svelte.ts. */
export type Indicator = { message: string } & IndicatorDetail;

export interface TranscodeAssessment {
  verdict: Verdict;
  /** Strength of the evidence behind the *stated* verdict, 0-1. **Not a probability**: these
   * are heuristic weights tuned on a twenty-fixture corpus, so the UI shows a weak/moderate/
   * strong band and keeps the number for the exported report. null for `declared_lossy`,
   * where the container states the answer and there is nothing to be more or less sure of. */
  confidence_score: number | null;
  indicators: Indicator[];
}

export interface EncoderTagMatch {
  tag_key: string;
  tag_value: string;
  matched_pattern: string;
}

export interface BitDepthAnalysis {
  declared_bit_depth: number | null;
  /** Smallest bit depth that explains ~all samples. Equal to declared_bit_depth in the
   * normal case; lower means the file was likely zero-padded to look deeper than it is.
   * null means unverifiable — no declared depth, a declared depth above the 24 bits an f32
   * decode can carry, or too little non-silent audio to judge. */
  effective_bit_depth: number | null;
  /** Fraction of samples that were non-zero and therefore carried evidence. Silence sits on
   * every quantization grid at once, so the check runs on active samples only. */
  active_sample_ratio: number;
}

/** The sample-rate counterpart to BitDepthAnalysis: a file resampled up to a hi-res rate
 * it never earns. Lossless throughout, so invisible to the transcode verdict. */
export interface SampleRateAnalysis {
  declared_sample_rate_hz: number;
  /** null when the bandwidth was not measured at all — never a Nyquist stand-in. */
  content_bandwidth_hz: number | null;
  /** content_bandwidth_hz as a fraction of the declared Nyquist; null when unmeasured. */
  bandwidth_ratio: number | null;
  /** Set only on a bandwidth that was actually measured. */
  likely_upsampled: boolean;
  /** Smallest standard rate that would carry this content losslessly, when flagged. */
  sufficient_sample_rate_hz: number | null;
}

/** AAC encoder frame-grid alignment. Mirrors `MdctGridAnalysis` in mdct_grid.rs.
 *
 * Unlike the stereo image, this one *does* feed the verdict: an alignment at which the
 * file's own MDCT coefficients collapse is an encoder's quantization grid, which lossless
 * audio has no reason to exhibit. Covers AAC only — MP3's hybrid filterbank is not a plain
 * MDCT and cannot be inverted this way. */
export interface MdctGridAnalysis {
  grid_detected: boolean;
  /** Robust standard deviations above the file's own median offset. */
  z_score: number;
  /** Winning offset in samples, 0..1024. */
  frame_offset: number;
  zero_fraction_at_offset: number;
  zero_fraction_baseline: number;
  /** false when the file was too short or too quiet to sweep; other fields are then void. */
  analyzed: boolean;
  /** One byte per candidate offset (1024 of them), each the zero-fraction there scaled
   * against the strongest, base64-encoded. Drawn as-is by MdctGrid.svelte: the shape is the
   * evidence — a low uneven ridge for lossless, a flat floor with one spike for AAC. */
  sweep_profile_base64: string;
}

export interface AnalysisResult {
  /** Which build of the pipeline produced these numbers. Thresholds and verdict logic move
   * between releases; an exported report is otherwise uninterpretable later. */
  analysis_version: string;
  decode_status: DecodeStatus;
  file_info: FileInfo;
  signal_analysis: SignalAnalysis;
  dynamic_range: DynamicRangeResult;
  spectral_analysis: SpectralAnalysis;
  transcode_assessment: TranscodeAssessment;
  encoder_tag_matches: EncoderTagMatch[];
  bit_depth_analysis: BitDepthAnalysis;
  sample_rate_analysis: SampleRateAnalysis;
  /** null for anything that is not exactly two channels. */
  stereo_analysis: StereoAnalysis | null;
  mdct_grid: MdctGridAnalysis;
}

export function analyzeFile(path: string): Promise<AnalysisResult> {
  return invoke<AnalysisResult>("analyze_file", { path });
}

/** Everything the transport needs to draw itself, returned by every player call.
 *
 * `position_seconds` is derived from the sample index handed to the audio device, and
 * `duration_seconds` from the decoder — the same number `file_info.duration_seconds` and the
 * spectrogram's time axis use. They are one clock now. The `<audio>` element kept a second,
 * disagreeing one, which is where the wrong seeks, the drifting counter and the early stops
 * all came from. See src-tauri/src/player.rs. */
export interface PlaybackState {
  position_seconds: number;
  duration_seconds: number;
  playing: boolean;
  /** The playhead reached the end of the track. */
  ended: boolean;
  /** No track is loaded — the audio device could not be opened, or nothing was analyzed. */
  loaded: boolean;
  /** Why nothing is loaded, when a load was attempted and failed. null when playback simply
   * has not been asked for yet. */
  unavailable_reason: string | null;
}

/** Playback is loaded by `analyzeFile` from the very samples it analyzed, so there is no
 * separate "authorize" step and no URL: nothing is served to the webview at all. */
export function playerPlay(): Promise<PlaybackState> {
  return invoke<PlaybackState>("player_play");
}

export function playerPause(): Promise<PlaybackState> {
  return invoke<PlaybackState>("player_pause");
}

/** Seconds from the start of the track, in the same units as everything else on screen. */
export function playerSeek(seconds: number): Promise<PlaybackState> {
  return invoke<PlaybackState>("player_seek", { seconds });
}

export function playerSetVolume(volume: number): Promise<PlaybackState> {
  return invoke<PlaybackState>("player_set_volume", { volume });
}

/** Polled while playing to drive the playhead. Cheap on the Rust side: two atomic loads. */
export function playerState(): Promise<PlaybackState> {
  return invoke<PlaybackState>("player_state");
}

/** Qualitative band for a confidence weight.
 *
 * The backend's numbers (0.25, 0.3, 0.7, 0.9, …) are heuristic weights tuned on a small
 * corpus, not calibrated probabilities. Rendering "90 %" claimed a precision no held-out
 * validation set supports; the raw value stays in the exported JSON where a reader can see
 * it for what it is. */
export type EvidenceStrength = "weak" | "moderate" | "strong";

export function evidenceStrength(confidence: number): EvidenceStrength {
  if (confidence >= 0.85) return "strong";
  if (confidence >= 0.6) return "moderate";
  return "weak";
}

export function exportReport(path: string, json: string): Promise<void> {
  return invoke<void>("export_report", { path, json });
}
