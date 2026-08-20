/** UI translations. French is the app's default language — see the discreet toggle in the
 * topbar (+page.svelte) — with English as the one alternative. Shared reactive state (a
 * Svelte 5 rune at module scope) rather than a store: every component that reads `lang` or
 * `t` re-renders when the toggle fires, with no provider/context wiring needed for a
 * single-page app.
 *
 * Backend error strings (from `analyzeFile`/Rust) are not covered here — they are
 * technical/debug text, not part of this catalogue.
 */

export type Lang = "fr" | "en";

const STORAGE_KEY = "nyquist-lang";

function detectInitialLang(): Lang {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "fr" || saved === "en") return saved;
  } catch {
    /* localStorage unavailable (private mode, non-browser render) — fall through. */
  }
  return "fr";
}

/** Module-scope rune: reactive across every importer, no store/context boilerplate. */
export const langState = $state<{ current: Lang }>({ current: "fr" });

let initialized = false;

/** Called once from the page's `onMount` — reading `localStorage` during module init would
 * run before Tauri's webview is ready and isn't needed for the default (French) case. */
export function initLang() {
  if (initialized) return;
  initialized = true;
  langState.current = detectInitialLang();
}

export function setLang(lang: Lang) {
  langState.current = lang;
  try {
    localStorage.setItem(STORAGE_KEY, lang);
  } catch {
    /* Best-effort persistence only. */
  }
}

export function toggleLang() {
  setLang(langState.current === "fr" ? "en" : "fr");
}

interface Dict {
  brand: { tagline: string };
  actions: { openAnother: string; exportJson: string; switchTheme: string; switchLang: string };
  dropzone: { title: string; subtitle: string; chooseFile: string };
  loading: { text: string; hint: string };
  dragOverlay: string;
  verdict: {
    probablyAuthentic: { label: string; blurb: string };
    probablyTranscoded: { label: string; blurb: string };
    indeterminate: { label: string; blurb: string };
    confidence: string;
  };
  findings: {
    checksumMismatchTitle: string;
    checksumMismatchDetail: string;
    damagedPacketsTitle: (n: number) => string;
    damagedPacketsDetail: string;
    bitDepthPaddingTitle: (declared: number, effective: number) => string;
    bitDepthPaddingDetail: (effective: number) => string;
    upsampledTitle: (declaredKHz: string, usedKHz: string) => string;
    upsampledDetail: (pct: string, sufficientKHz: string | null) => string;
    clippedSamplesTitle: (n: string) => string;
    clippedSamplesDetail: string;
  };
  file: {
    bandwidthUsed: string;
    bandwidthPhrase: (used: string, nyquist: string) => string;
  };
  spectrum: {
    title: string;
    bandwidth: string;
    rolloffSteepness: string;
    noEdgeFound: string;
    steepnessValue: (db: string, hz: string) => string;
    note: string;
  };
  loudness: {
    title: string;
    integratedLoudness: string;
    na: string;
    lufsTargetNote: string;
    truePeak: string;
    clipWarnNote: string;
    headroomNote: string;
    loudnessRange: string;
    peakRms: string;
  };
  dynamics: {
    title: string;
    dynamicRange: string;
    drLabel: (dr: number) => string;
    drNote: (label: string) => string;
    clippedSamples: string;
    table: { ch: string; peak: string; rms: string; crest: string; dr: string; clipped: string };
    channelsNote: string;
  };
  file2: {
    title: string;
    container: string;
    codec: string;
    sampleRate: string;
    nyquist: string;
    bitDepth: string;
    channels: string;
    duration: string;
    size: string;
    avgBitrate: string;
    samples: string;
    integrity: string;
    integrityVerified: string;
    integrityMismatch: string;
    integrityNoChecksum: string;
    integrityUnavailable: string;
    stereo: string;
  };
  disclaimer: string;
  player: { play: string; pause: string; playbackPosition: string; mute: string; unmute: string; volume: string };
  spectrogram: { seekAria: string; canvasAria: string; rawCutoff: (freq: string) => string };
}

const fr: Dict = {
  brand: { tagline: "Ce fichier est-il vraiment ce qu'il prétend être ?" },
  actions: {
    openAnother: "Ouvrir un autre",
    exportJson: "Exporter en JSON",
    switchTheme: "Changer de thème",
    switchLang: "Afficher l'application en anglais"
  },
  dropzone: {
    title: "Déposez un fichier audio ici",
    subtitle: "FLAC, ALAC, WAV, MP3, AAC ou OGG. Rien ne quitte votre machine.",
    chooseFile: "Choisir un fichier"
  },
  loading: {
    text: "Décodage et analyse en cours…",
    hint: "Passes FFT et de sonie sur l'intégralité de chaque échantillon."
  },
  dragOverlay: "Déposer pour analyser",
  verdict: {
    probablyAuthentic: {
      label: "Probablement authentique",
      blurb: "Aucune empreinte d'encodeur trouvée dans le spectre."
    },
    probablyTranscoded: {
      label: "Probablement transcodé",
      blurb: "Ceci ressemble à de l'audio avec perte encapsulé dans un conteneur sans perte."
    },
    indeterminate: {
      label: "Indéterminé",
      blurb: "Pas assez d'éléments dans un sens ou dans l'autre. C'est une réponse à part entière, pas un échec."
    },
    confidence: "confiance"
  },
  findings: {
    checksumMismatchTitle: "Somme de contrôle invalide",
    checksumMismatchDetail:
      "L'audio ne correspond pas à la somme de contrôle stockée dans le fichier. Il a été tronqué, modifié ou corrompu depuis sa création.",
    damagedPacketsTitle: (n) => (n > 1 ? `${n} paquets endommagés ignorés` : `${n} paquet endommagé ignoré`),
    damagedPacketsDetail:
      "Une partie du fichier n'a pas pu être décodée. Chaque mesure ci-dessous décrit l'audio qui a survécu, pas la piste entière.",
    bitDepthPaddingTitle: (declared, effective) => `Conteneur ${declared} bits contenant de l'audio ${effective} bits`,
    bitDepthPaddingDetail: (effective) =>
      `Chaque échantillon tombe exactement sur la grille de quantification à ${effective} bits : la résolution supplémentaire ne porte aucune information. Le fichier a été rembourré, pas réellement remasterisé.`,
    upsampledTitle: (declaredKHz, usedKHz) => `${declaredKHz} kHz déclarés, ${usedKHz} kHz utilisés`,
    upsampledDetail: (pct, sufficientKHz) =>
      `Le contenu s'arrête à ${pct}% de la bande passante que cette fréquence d'échantillonnage est censée porter${
        sufficientKHz ? `. Un fichier à ${sufficientKHz} kHz contiendrait tout, sans perte` : ""
      }. L'audio est intact — c'est la fréquence d'échantillonnage annoncée qui est gonflée.`,
    clippedSamplesTitle: (n) => `${n} échantillons écrêtés`,
    clippedSamplesDetail: "Échantillons plaqués au plein échelle, là où la forme d'onde a été aplatie plutôt que reproduite."
  },
  file: {
    bandwidthUsed: "Bande passante utilisée",
    bandwidthPhrase: (used, nyquist) => `${used} sur ${nyquist}`
  },
  spectrum: {
    title: "Spectre",
    bandwidth: "Bande passante",
    rolloffSteepness: "Pente de coupure",
    noEdgeFound: "aucune coupure détectée",
    steepnessValue: (db, hz) => `${db} dB/kHz à ${hz}`,
    note: "La bande passante indique où le contenu s'arrête — le bord du filtre passe-bas s'il y en a un, sinon Nyquist. La pente est ce qui distingue un encodeur d'un mixage sombre : le filtre d'un codec tombe à pic, un choix de mastering s'estompe progressivement. Cliquez sur le spectrogramme pour déplacer la lecture à cet endroit."
  },
  loudness: {
    title: "Sonie",
    integratedLoudness: "Sonie intégrée",
    na: "n/d",
    lufsTargetNote: "le repère marque la cible streaming de -14 LUFS",
    truePeak: "Crête réelle",
    clipWarnNote: "au-dessus du plein échelle — écrêtera lors d'un ré-échantillonnage ou d'un ré-encodage",
    headroomNote: "le repère marque la marge de -1 dBTP demandée par l'EBU R128",
    loudnessRange: "Plage de sonie",
    peakRms: "Crête / RMS"
  },
  dynamics: {
    title: "Dynamique",
    dynamicRange: "Plage dynamique",
    drLabel: (dr) => (dr >= 14 ? "ample" : dr >= 12 ? "bonne" : dr >= 8 ? "modérée" : "fortement compressée"),
    drNote: (label) => `${label} — échelle DR Pleasurize, celle qu'utilise la base de données loudness-war`,
    clippedSamples: "Échantillons écrêtés",
    table: { ch: "Ch", peak: "Crête", rms: "RMS", crest: "Crest", dr: "DR", clipped: "Écrêtés" },
    channelsNote:
      "Le facteur de crête est un simple rapport crête/RMS. Le DR est l'algorithme Pleasurize par blocs. Ils mesurent des choses différentes et ne sont pas censés concorder."
  },
  file2: {
    title: "Fichier",
    container: "Conteneur",
    codec: "Codec",
    sampleRate: "Fréquence d'échantillonnage",
    nyquist: "Nyquist",
    bitDepth: "Résolution",
    channels: "Canaux",
    duration: "Durée",
    size: "Taille",
    avgBitrate: "Débit moyen",
    samples: "Échantillons",
    integrity: "Intégrité",
    integrityVerified: "Somme de contrôle vérifiée",
    integrityMismatch: "Somme de contrôle invalide",
    integrityNoChecksum: "Aucune somme de contrôle stockée",
    integrityUnavailable: "Non disponible pour ce codec",
    stereo: "stéréo"
  },
  disclaimer:
    "Nyquist rapporte ce qu'il peut mesurer et le dit clairement quand ce n'est pas suffisant. Le verdict de transcodage repose surtout sur la forme de la pente spectrale, qui ne peut pas détecter un encodage transparent comme LAME V0 ou AAC 256 — un résultat propre n'est pas une preuve de provenance.",
  player: {
    play: "Lecture",
    pause: "Pause",
    playbackPosition: "Position de lecture",
    mute: "Couper le son",
    unmute: "Rétablir le son",
    volume: "Volume"
  },
  spectrogram: {
    seekAria: "Cliquer pour déplacer la lecture",
    canvasAria: "Spectrogramme",
    rawCutoff: (freq) => `coupure brute ~${freq}Hz`
  }
};

const en: Dict = {
  brand: { tagline: "Is this file what it says it is?" },
  actions: {
    openAnother: "Open another",
    exportJson: "Export JSON",
    switchTheme: "Switch theme",
    switchLang: "Switch the app to French"
  },
  dropzone: {
    title: "Drop an audio file here",
    subtitle: "FLAC, ALAC, WAV, MP3, AAC or OGG. Nothing leaves your machine.",
    chooseFile: "Choose a file"
  },
  loading: {
    text: "Decoding and analyzing…",
    hint: "Full-length FFT and loudness passes over every sample."
  },
  dragOverlay: "Drop to analyze",
  verdict: {
    probablyAuthentic: {
      label: "Probably authentic",
      blurb: "No encoder fingerprint found in the spectrum."
    },
    probablyTranscoded: {
      label: "Probably transcoded",
      blurb: "This looks like lossy audio wrapped in a lossless container."
    },
    indeterminate: {
      label: "Inconclusive",
      blurb: "Not enough evidence either way. That is a real answer, not a failure."
    },
    confidence: "confidence"
  },
  findings: {
    checksumMismatchTitle: "Checksum mismatch",
    checksumMismatchDetail:
      "The audio does not match the checksum stored inside the file. It has been truncated, edited, or corrupted since it was created.",
    damagedPacketsTitle: (n) => `${n} damaged packet${n > 1 ? "s" : ""} skipped`,
    damagedPacketsDetail:
      "Part of the file could not be decoded. Every measurement below describes the audio that survived, not the whole track.",
    bitDepthPaddingTitle: (declared, effective) => `${declared}-bit container holding ${effective}-bit audio`,
    bitDepthPaddingDetail: (effective) =>
      `Every sample lands exactly on the ${effective}-bit quantization grid, so the extra depth carries no information. The file was padded, not remastered.`,
    upsampledTitle: (declaredKHz, usedKHz) => `${declaredKHz} kHz declared, ${usedKHz} kHz used`,
    upsampledDetail: (pct, sufficientKHz) =>
      `Content stops at ${pct}% of the bandwidth this sample rate exists to carry${
        sufficientKHz ? `. A ${sufficientKHz} kHz file would hold all of it losslessly` : ""
      }. The audio is intact — the sample rate on the label is inflated.`,
    clippedSamplesTitle: (n) => `${n} clipped samples`,
    clippedSamplesDetail: "Samples pinned at full scale, where the waveform was flattened rather than reproduced."
  },
  file: {
    bandwidthUsed: "Bandwidth used",
    bandwidthPhrase: (used, nyquist) => `${used} of ${nyquist}`
  },
  spectrum: {
    title: "Spectrum",
    bandwidth: "Bandwidth",
    rolloffSteepness: "Rolloff steepness",
    noEdgeFound: "no edge found",
    steepnessValue: (db, hz) => `${db} dB/kHz @ ${hz}`,
    note: "Bandwidth is where content stops — the lowpass edge if there is one, otherwise Nyquist. Steepness is what separates an encoder from a dark mix: a codec's lowpass falls off a cliff, a mastering choice slopes away. Click the spectrogram to jump playback there."
  },
  loudness: {
    title: "Loudness",
    integratedLoudness: "Integrated loudness",
    na: "n/a",
    lufsTargetNote: "tick marks the -14 LUFS streaming target",
    truePeak: "True peak",
    clipWarnNote: "above full scale — will clip when resampled or re-encoded",
    headroomNote: "tick marks the -1 dBTP headroom EBU R128 asks for",
    loudnessRange: "Loudness range",
    peakRms: "Peak / RMS"
  },
  dynamics: {
    title: "Dynamics",
    dynamicRange: "Dynamic range",
    drLabel: (dr) => (dr >= 14 ? "wide" : dr >= 12 ? "good" : dr >= 8 ? "moderate" : "heavily compressed"),
    drNote: (label) => `${label} — Pleasurize DR scale, the one the loudness-war database uses`,
    clippedSamples: "Clipped samples",
    table: { ch: "Ch", peak: "Peak", rms: "RMS", crest: "Crest", dr: "DR", clipped: "Clipped" },
    channelsNote:
      "Crest factor is a plain peak-to-RMS ratio. DR is the block-based Pleasurize algorithm. They measure different things and are not meant to match."
  },
  file2: {
    title: "File",
    container: "Container",
    codec: "Codec",
    sampleRate: "Sample rate",
    nyquist: "Nyquist",
    bitDepth: "Bit depth",
    channels: "Channels",
    duration: "Duration",
    size: "Size",
    avgBitrate: "Avg. bitrate",
    samples: "Samples",
    integrity: "Integrity",
    integrityVerified: "Checksum verified",
    integrityMismatch: "Checksum mismatch",
    integrityNoChecksum: "No checksum stored",
    integrityUnavailable: "Not available for this codec",
    stereo: "stereo"
  },
  disclaimer:
    "Nyquist reports what it can measure and says so when that is not enough. The transcode verdict rests mainly on the shape of the spectral rolloff, which cannot see a transparent encode such as LAME V0 or AAC 256 — a clean result is not proof of provenance.",
  player: {
    play: "Play",
    pause: "Pause",
    playbackPosition: "Playback position",
    mute: "Mute",
    unmute: "Unmute",
    volume: "Volume"
  },
  spectrogram: {
    seekAria: "Click to seek playback position",
    canvasAria: "Spectrogram",
    rawCutoff: (freq) => `raw cutoff ~${freq}Hz`
  }
};

const dicts: Record<Lang, Dict> = { fr, en };

/** Reads the current dictionary. A plain function rather than a stored `$derived` so it can
 * be called directly from markup and from `Spectrogram.svelte`'s own template without
 * threading a prop through — reading `langState.current` inside it is what makes each call
 * site reactive, the same way any rune read inside a template expression is. */
export function t(): Dict {
  return dicts[langState.current];
}

/** Locale-aware number formatting (comma decimals in French) for the two formatters used
 * throughout the dashboard. Reads `langState.current` at call time for the same reason as
 * `t()` above. */
export function fmtNumber(value: number, digits = 1): string {
  return new Intl.NumberFormat(langState.current === "fr" ? "fr-FR" : "en-US", {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits
  }).format(value);
}
