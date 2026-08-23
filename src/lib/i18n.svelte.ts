/** UI translations. French is the app's default language — see the discreet toggle in the
 * topbar (+page.svelte) — with English as the one alternative. Shared reactive state (a
 * Svelte 5 rune at module scope) rather than a store: every component that reads `lang` or
 * `t` re-renders when the toggle fires, with no provider/context wiring needed for a
 * single-page app.
 *
 * The verdict's evidence list is the one part not authored here. Those sentences come from
 * `transcode_detect.rs` as a code plus its measurements, and `Dict.indicator` re-composes
 * them: English by handing back the backend's own prose (so the UI, the CLI and an exported
 * report all state the verdict identically), French by rewriting it from the same numbers.
 *
 * Backend error strings (from `analyzeFile`/Rust) are not covered here — they are
 * technical/debug text, not part of this catalogue.
 */

import type { Indicator } from "$lib/api";

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
  /** Names shown in the OS file picker / save dialog, which is chrome we still own. */
  dialogs: { audioFiles: string; jsonFiles: string };
  dropzone: { title: string; subtitle: string; chooseFile: string };
  loading: { text: string; hint: string };
  dragOverlay: string;
  verdict: {
    probablyAuthentic: { label: string; blurb: string };
    probablyTranscoded: { label: string; blurb: string };
    indeterminate: { label: string; blurb: string };
    confidence: string;
  };
  /** Renders one piece of the verdict's evidence. Exhaustive over `Indicator["code"]` in
   * both languages — a new indicator variant in transcode_detect.rs fails `npm run check`
   * until it is translated, so backend evidence can never silently reach a French UI in
   * English. */
  indicator: (indicator: Indicator) => string;
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
    /** "24-bit" vs "24 bits" — the hyphenated compound is English-only. */
    bits: (depth: number) => string;
    channelCount: (channels: number) => string;
    /** Megabyte: "MB" in English, "Mo" (mégaoctet) in French. */
    megabytes: (value: string) => string;
  };
  stereo: {
    title: string;
    correlation: string;
    correlationNote: string;
    width: string;
    dualMono: string;
    dualMonoNote: string;
    effectivelyMono: string;
    phaseRisk: string;
    phaseRiskNote: string;
    perBand: string;
    bandName: (name: string) => string;
    note: string;
  };
  spectralDetail: {
    title: string;
    stability: string;
    stabilityNote: string;
    stopbandDepth: string;
    stopbandNote: string;
    noStopband: string;
    bandLevels: string;
    bandLevelsNote: string;
    toNyquist: string;
  };
  disclaimer: string;
  player: { play: string; pause: string; playbackPosition: string; mute: string; unmute: string; volume: string };
  spectrogram: { seekAria: string; canvasAria: string; rawCutoff: (freq: string) => string };
  mdct: {
    title: string;
    detected: string;
    clear: string;
    notAnalyzed: string;
    offset: string;
    zeroed: string;
    baseline: string;
    strength: string;
    axisOffset: string;
    chartAria: string;
    note: string;
  };
}

const fr: Dict = {
  brand: { tagline: "Ce fichier est-il vraiment ce qu'il prétend être ?" },
  actions: {
    openAnother: "Ouvrir un autre",
    exportJson: "Exporter en JSON",
    switchTheme: "Changer de thème",
    switchLang: "Afficher l'application en anglais"
  },
  dialogs: { audioFiles: "Fichiers audio", jsonFiles: "Fichiers JSON" },
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
  indicator: (i) => {
    switch (i.code) {
      case "encoder_tag_matched":
        return `Le tag d'encodeur « ${i.tag_key} » vaut « ${i.tag_value} », ce qui correspond à l'encodeur exclusivement avec perte « ${i.matched_pattern} »${
          i.additional_matches > 0
            ? i.additional_matches > 1
              ? `, plus ${i.additional_matches} autres tags correspondants`
              : ", plus un autre tag correspondant"
            : ""
        }.`;
      case "tag_is_only_evidence":
        return "Le spectre seul n'a rien permis de conclure : ce verdict repose donc sur le tag — qui peut être obsolète, recopié depuis un fichier source, ou tout simplement faux.";
      case "tag_contradicts_spectrum":
        return "Ceci contredit la mesure spectrale ci-dessus, qui n'a trouvé aucune coupure d'encodeur. Soit le tag est un reliquat d'une étape antérieure de l'histoire du fichier et l'audio est réellement sans perte, soit il s'agissait d'un encodage avec perte transparent que cette méthode ne peut pas voir. Rapporté comme indéterminé plutôt que de laisser l'un des deux signaux l'emporter sur l'autre.";
      case "invalid_sample_rate":
        return "Fréquence d'échantillonnage invalide ; impossible d'évaluer le contenu spectral.";
      case "sharp_rolloff":
        return `Coupure spectrale franche (~${fmtNumber(i.steepness_db_per_khz, 0)} dB/kHz) autour de ${fmtNumber(i.edge_khz, 1)} kHz — assez raide pour correspondre au filtre passe-bas d'un encodeur avec perte plutôt qu'à un contenu de mixage ou de mastering naturel (la pente naturelle mesurée reste bien en dessous de 20 dB/kHz sur tout le corpus de test du projet, réel comme synthétique).`;
      case "no_encoder_lowpass":
        return `Aucun passe-bas d'encodeur trouvé : le spectre a été balayé depuis ${fmtNumber(i.scanned_from_khz, 0)} kHz jusqu'à la fréquence de Nyquist de ${fmtNumber(i.nyquist_khz, 1)} kHz, et aucun point n'a montré la chute nette vers une bande vide durable que laisse un codec avec perte.`;
      case "transparent_encode_unseen":
        return "Cela n'exclut pas un encodage avec perte transparent (par ex. LAME V0, AAC 256 kbps) — le corpus du projet montre que ceux-ci se mesurent de façon indiscernable du sans perte par cette méthode. La confiance est plafonnée en conséquence.";
      case "gradual_rolloff":
        return `Le contenu s'arrête autour de ${fmtNumber(i.cutoff_khz, 1)} kHz, mais la transition y est progressive (~${fmtNumber(i.steepness_db_per_khz, 0)} dB/kHz) plutôt que le mur quasi vertical que produit un codec. C'est compatible avec un master volontairement sombre, un report de vinyle ou de bande, ou un encodage avec perte dont cette méthode ne peut pas distinguer le filtre — pas de quoi trancher dans un sens ou dans l'autre.`;
      case "mdct_grid_aligned":
        return `Les coefficients MDCT du fichier s'effondrent à un alignement de trame précis (décalage ${i.frame_offset}, ${fmtNumber(i.z_score, 0)} écarts-types au-dessus de ce que fait ce même fichier à tous les autres décalages) : ${fmtNumber(i.zero_percent, 1)} % des coefficients y sont annulés, contre ${fmtNumber(i.baseline_percent, 1)} % ailleurs. C'est la grille de quantification d'un encodeur AAC. De l'audio sans perte n'a aucun alignement de ce genre.`;
      case "mdct_grid_clear":
        return "Le balayage de la grille MDCT n'a trouvé aucun alignement d'encodeur, ce qui écarte une source AAC — y compris aux réglages transparents qu'une mesure spectrale ne peut pas voir. Cela ne dit rien du MP3, dont le banc de filtres hybride n'est pas inversible par cette méthode : le point aveugle se rétrécit, il ne se referme pas.";
      case "bandwidth_above_cd_ceiling":
        return `Le contenu monte jusqu'à ${fmtNumber(i.cutoff_khz, 1)} kHz, au-dessus du plafond de 22,05 kHz que peut porter n'importe quelle source à la fréquence du CD. Cela écarte le chemin de transcodage le plus courant par la mesure, et non par absence de preuve.`;
    }
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
    stereo: "stéréo",
    bits: (depth) => `${depth} bits`,
    channelCount: (channels) => `${channels} canaux`,
    megabytes: (value) => `${value} Mo`
  },
  stereo: {
    title: "Image stéréo",
    correlation: "Corrélation L/R",
    correlationNote: "1 = canaux identiques, 0 = contenus indépendants, négatif = hors phase",
    width: "Side / Mid",
    dualMono: "Dual-mono",
    dualMonoNote: "Les deux canaux sont identiques au bit près : du mono dans un conteneur stéréo.",
    effectivelyMono: "Largeur négligeable",
    phaseRisk: "Risque en mono",
    phaseRiskNote: "Corrélation négative : sommer ce fichier en mono annulera du contenu au lieu de simplement le resserrer. Typique d'un élargissement stéréo artificiel.",
    perBand: "Largeur par bande",
    bandName: (name) => (name === "low" ? "Graves" : name === "mid" ? "Médiums" : "Aigus"),
    note: "Information sur le fichier, pas un indice de transcodage. Les encodeurs avec perte laissent bien une signature dans l'image stéréo, mais mesurée sur le corpus de ce projet elle ne sépare pas un encodage transparent de sa source sans perte — voir stereo.rs."
  },
  spectralDetail: {
    title: "Détail spectral",
    stability: "Stabilité de la coupure",
    stabilityNote: "dispersion du haut du contenu sur la durée du morceau — mesure rapportée, jamais utilisée dans le verdict : un lowpass de mastering est aussi fixe que celui d'un codec",
    stopbandDepth: "Profondeur du stopband",
    stopbandNote: "de combien la zone au-dessus de la coupure descend sous celle du dessous",
    noStopband: "aucune coupure détectée",
    bandLevels: "Niveaux par bande",
    bandLevelsNote: "Niveau moyen de chaque bande, relatif à la plus forte du fichier. C'est la forme spectrale dont le verdict est tiré : un mur d'encodeur y apparaît comme une chute brutale entre deux bandes voisines, un master sombre comme une pente régulière.",
    toNyquist: "Nyquist"
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
  },
  mdct: {
    title: "Grille MDCT",
    detected: "Grille d'encodeur AAC détectée",
    clear: "Aucun alignement d'encodeur",
    notAnalyzed: "Fichier trop court ou trop calme pour être balayé",
    offset: "Décalage",
    zeroed: "Coefficients annulés",
    baseline: "ailleurs",
    strength: "Écart",
    axisOffset: "décalage de trame (échantillons)",
    chartAria: "Profil du balayage de la grille MDCT",
    note: "Chaque colonne est un décalage de trame possible ; sa hauteur, la part de coefficients MDCT annulés à ce décalage. Un fichier sans perte se comporte pareil partout — un relief irrégulier et bas. Un encodeur AAC laisse sa grille : un pic unique, à sa position de trame. C'est une propriété structurelle du fichier, indépendante de la forme du spectre, et c'est pourquoi elle voit ce que la pente de coupure ne voit pas. Le MP3 n'est pas couvert : son banc de filtres hybride n'est pas une MDCT simple."
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
  dialogs: { audioFiles: "Audio files", jsonFiles: "JSON files" },
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
  // The backend authors these in English already. Handing its own prose straight back keeps
  // the app, `nyquist-cli` and an exported report word-for-word identical, and leaves the
  // wording with a single owner: `IndicatorDetail::english` in transcode_detect.rs.
  indicator: (i) => i.message,
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
    stereo: "stereo",
    bits: (depth) => `${depth}-bit`,
    channelCount: (channels) => `${channels}ch`,
    megabytes: (value) => `${value} MB`
  },
  stereo: {
    title: "Stereo image",
    correlation: "L/R correlation",
    correlationNote: "1 = identical channels, 0 = unrelated content, negative = out of phase",
    width: "Side / Mid",
    dualMono: "Dual mono",
    dualMonoNote: "The two channels are bit-identical: mono content in a stereo container.",
    effectivelyMono: "Negligible width",
    phaseRisk: "Mono risk",
    phaseRiskNote: "Negative correlation: summing this file to mono will cancel content rather than just narrow it. Typically an artificially widened master.",
    perBand: "Width per band",
    bandName: (name) => (name === "low" ? "Low" : name === "mid" ? "Mid" : "High"),
    note: "Information about the file, not a transcode signal. Lossy encoders do leave a stereo fingerprint, but measured across this project's corpus it does not separate a transparent encode from its lossless source — see stereo.rs."
  },
  spectralDetail: {
    title: "Spectral detail",
    stability: "Cutoff stability",
    stabilityNote: "how much the top of the content wanders across the track — reported, never scored: a mastering lowpass is as fixed as a codec's",
    stopbandDepth: "Stopband depth",
    stopbandNote: "how far the region above the cutoff sits below the region under it",
    noStopband: "no edge found",
    bandLevels: "Band levels",
    bandLevelsNote: "Average level of each band, relative to the loudest in the file. This is the spectral shape the verdict is drawn from: an encoder wall shows up as an abrupt fall between neighbouring bands, a dark master as a steady slope.",
    toNyquist: "Nyquist"
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
  },
  mdct: {
    title: "MDCT grid",
    detected: "AAC encoder grid detected",
    clear: "No encoder alignment",
    notAnalyzed: "File too short or too quiet to sweep",
    offset: "Offset",
    zeroed: "Coefficients zeroed",
    baseline: "elsewhere",
    strength: "Margin",
    axisOffset: "frame offset (samples)",
    chartAria: "MDCT grid sweep profile",
    note: "Each column is a candidate frame offset; its height is the share of MDCT coefficients reading as zeroed there. A lossless file behaves much the same at every offset — a low, uneven ridge. An AAC encoder leaves its grid behind: a single spike, at its own frame position. This is a structural property of the file, independent of the shape of its spectrum, which is why it sees what the rolloff measurement cannot. MP3 is not covered: its hybrid filterbank is not a plain MDCT."
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
