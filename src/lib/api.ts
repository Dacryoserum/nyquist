import { invoke } from "@tauri-apps/api/core";

export interface ChannelStats {
  channel: number;
  peak_dbfs: number;
  rms_dbfs: number;
  crest_factor_db: number;
  clipping_count: number;
}

export interface SignalAnalysis {
  peak_dbfs: number;
  true_peak_dbtp: number;
  rms_dbfs: number;
  lufs_integrated: number | null;
  /** EBU Tech 3342 Loudness Range, in LU — companion metric to lufs_integrated. */
  loudness_range_lu: number | null;
  clipping_count_total: number;
  per_channel: ChannelStats[];
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
  /** Packets that failed to decode and were skipped. Non-zero means every measurement in
   * this report describes damaged, incomplete audio. */
  decode_errors: number;
}

export interface SpectrogramData {
  time_bin_count: number;
  frequency_bin_count: number;
  max_frequency_hz: number;
  duration_seconds: number;
  /** Row-major [time][frequency], u8 dB-mapped intensity, base64-encoded. */
  intensity_base64: string;
}

export interface SpectralAnalysis {
  /** Where content stops: the lowpass edge if there is one, otherwise Nyquist. Raw
   * measurement, not a transcode verdict — see spectral.rs module docs. */
  spectral_cutoff_hz: number;
  rolloff_steepness_db_per_khz: number;
  /** Position of the codec-like edge, or null when no lowpass exists anywhere above 8 kHz
   * — the latter being the evidence behind a "probably authentic" verdict. */
  encoder_edge_hz: number | null;
  /** Same length/time alignment as spectrogram.time_bin_count. */
  cutoff_over_time_hz: number[];
  spectrogram: SpectrogramData;
}

export type Verdict = "probably_authentic" | "probably_transcoded" | "indeterminate";

export interface TranscodeAssessment {
  verdict: Verdict;
  /** Confidence in the *stated* verdict, 0-1 — deliberately conservative, see
   * transcode_detect.rs module docs. Not a probability of authenticity. */
  confidence_score: number;
  indicators: string[];
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
   * null means unverifiable — either no declared depth, or a declared depth above the 24
   * bits an f32 decode can carry. */
  effective_bit_depth: number | null;
}

/** The sample-rate counterpart to BitDepthAnalysis: a file resampled up to a hi-res rate
 * it never earns. Lossless throughout, so invisible to the transcode verdict. */
export interface SampleRateAnalysis {
  declared_sample_rate_hz: number;
  content_bandwidth_hz: number;
  /** content_bandwidth_hz as a fraction of the declared Nyquist. */
  bandwidth_ratio: number;
  likely_upsampled: boolean;
  /** Smallest standard rate that would carry this content losslessly, when flagged. */
  sufficient_sample_rate_hz: number | null;
}

export interface AnalysisResult {
  file_info: FileInfo;
  signal_analysis: SignalAnalysis;
  dynamic_range: DynamicRangeResult;
  spectral_analysis: SpectralAnalysis;
  transcode_assessment: TranscodeAssessment;
  encoder_tag_matches: EncoderTagMatch[];
  bit_depth_analysis: BitDepthAnalysis;
  sample_rate_analysis: SampleRateAnalysis;
}

export function analyzeFile(path: string): Promise<AnalysisResult> {
  return invoke<AnalysisResult>("analyze_file", { path });
}

/** Grants the webview permission to stream exactly this file via asset://, for playback. */
export function authorizePlayback(path: string): Promise<void> {
  return invoke<void>("authorize_playback", { path });
}

export function exportReport(path: string, json: string): Promise<void> {
  return invoke<void>("export_report", { path, json });
}
