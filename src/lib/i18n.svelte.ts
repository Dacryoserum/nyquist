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

import type { DecodeStatus, Indicator } from "$lib/api";

/** Display name for a codec short name. Acronyms stay upper-case, words stay capitalized —
 * "OPUS" and "VORBIS" read as shouting where "Opus" and "Vorbis" are just names. */
function codecName(codec: string): string {
  const known: Record<string, string> = {
    mp3: "MP3",
    mp2: "MP2",
    mp1: "MP1",
    aac: "AAC",
    vorbis: "Vorbis",
    opus: "Opus"
  };
  return known[codec] ?? codec.toUpperCase();
}

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
  loading: { text: string; hint: string; progress: (completed: number, total: number) => string };
  dragOverlay: string;
  verdict: {
    probablyAuthentic: { label: string; blurb: string };
    probablyTranscoded: { label: string; blurb: string };
    indeterminate: { label: string; blurb: string };
    /** Names the format, because "lossy" alone leaves the reader asking which one. */
    declaredLossy: { label: (codec: string) => string; blurb: string };
    confidence: string;
    /** Weak/moderate/strong rather than a percentage: the backend's numbers are heuristic
     * weights tuned on a small corpus, and rendering them as "90 %" claimed a precision no
     * held-out validation set supports. The raw value stays in the exported JSON. */
    strength: { weak: string; moderate: string; strong: string };
  };
  /** Renders one piece of the verdict's evidence. Exhaustive over `Indicator["code"]` in
   * both languages — a new indicator variant in transcode_detect.rs fails `npm run check`
   * until it is translated, so backend evidence can never silently reach a French UI in
   * English. */
  indicator: (indicator: Indicator) => string;
  findings: {
    checksumMismatchTitle: string;
    checksumMismatchDetail: string;
    incompleteDecodeTitle: (status: DecodeStatus) => string;
    incompleteDecodeDetail: string;
    bitDepthPaddingTitle: (declared: number, effective: number) => string;
    bitDepthPaddingDetail: (effective: number, activePct: string) => string;
    limitedBandwidthTitle: (declaredKHz: string, usedKHz: string) => string;
    limitedBandwidthDetail: (pct: string) => string;
    clippedRunsTitle: (runs: string, samples: string) => string;
    clippedRunsDetail: string;
  };
  /** Messages for failures the user can act on. Backend prose is English by design — it is
   * what the CLI prints and what an exported report preserves — so the common cases are
   * re-stated here with the original kept as detail. */
  errors: {
    cannotOpen: (raw: string) => string;
    unsupported: (raw: string) => string;
    noAudio: (raw: string) => string;
    playbackUnavailable: (raw: string) => string;
    exportSucceeded: (path: string) => string;
    exportFailed: (reason: string) => string;
  };
  file: {
    bandwidthUsed: string;
    bandwidthPhrase: (used: string, nyquist: string) => string;
    bandwidthUnmeasured: string;
  };
  spectrum: {
    title: string;
    bandwidth: string;
    rolloffSteepness: string;
    noEdgeFound: string;
    noLimitMeasured: string;
    steepnessValue: (db: string, hz: string) => string;
    note: string;
  };
  loudness: {
    title: string;
    integratedLoudness: string;
    na: string;
    lufsTargetNote: string;
    unknownLayoutNote: string;
    truePeak: string;
    clipWarnNote: string;
    headroomNote: string;
    /** Shown at 192 kHz and above, where ebur128 does no oversampling. */
    noOversamplingNote: string;
    loudnessRange: string;
    peakRms: string;
  };
  dynamics: {
    title: string;
    dynamicRange: string;
    drLabel: (dr: number) => string;
    drNote: (label: string) => string;
    clippedRuns: string;
    fullScaleNote: (samples: string) => string;
    table: { ch: string; peak: string; rms: string; crest: string; dr: string; fullScale: string; clipped: string };
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
  compare: {
    title: string;
    /** Button label — short, because it sits in a row of two other labelled actions. */
    action: string;
    add: string;
    exit: string;
    metric: string;
    note: string;
    loading: string;
    groupDeclared: string;
    groupSpectrum: string;
    yes: string;
    no: string;
    errorPrefix: string;
    /** Which of the two files the single shared player is bound to. */
    nowPlaying: (filename: string) => string;
    /** Screen-reader label for the marker on the leading value in a row. */
    leads: string;
    evidence: string;
  };
  disclaimer: string;
  player: { play: string; pause: string; playbackPosition: string; mute: string; unmute: string; volume: string };
  spectrogram: {
    seekAria: string;
    canvasAria: string;
    rawCutoff: (freq: string) => string;
    quiet: string;
    loud: string;
    palette: string;
    paletteName: (name: string) => string;
  };
  mdct: {
    title: string;
    detected: string;
    clear: string;
    notAnalyzed: string;
    offset: string;
    zeroed: string;
    baseline: string;
    strength: string;
    confirmation: string;
    window: string;
    sineWindow: string;
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
    progress: (completed, total) => `${completed}/${total} étapes terminées — les durées varient selon l'étape`,
    text: "Décodage et analyse en cours…",
    hint: "Passes FFT et de sonie sur l'intégralité de chaque échantillon."
  },
  dragOverlay: "Déposer pour analyser",
  verdict: {
    probablyAuthentic: {
      label: "Probablement authentique",
      blurb: "Un élément positif a été mesuré, qui écarte une source avec perte à la fréquence du CD."
    },
    probablyTranscoded: {
      label: "Probablement transcodé",
      blurb: "Ceci ressemble à de l'audio avec perte encapsulé dans un conteneur sans perte."
    },
    indeterminate: {
      label: "Indéterminé",
      blurb: "Les indices disponibles ne permettent pas d'établir la provenance. Ce n'est ni une garantie d'authenticité, ni une accusation."
    },
    declaredLossy: {
      label: (codec) => `Format avec perte (${codecName(codec)})`,
      blurb: "Ce fichier est dans un format avec perte et ne prétend pas le contraire. Il n'y a donc rien à démasquer — les mesures ci-dessous le décrivent quand même intégralement."
    },
    confidence: "force des indices",
    strength: { weak: "faible", moderate: "modérée", strong: "forte" }
  },
  indicator: (i) => {
    switch (i.code) {
      case "encoder_tag_matched":
        return `Le tag d'encodeur « ${i.tag_key} » vaut « ${i.tag_value} », ce qui correspond à l'encodeur exclusivement avec perte « ${i.matched_pattern} »${
          i.additional_tags > 0
            ? i.additional_tags > 1
              ? `, plus ${i.additional_tags} autres tags correspondants`
              : ", plus un autre tag correspondant"
            : ""
        }. Les tags stockés en fin de fichier (ID3v1, APEv2) ne sont pas lus : leur absence ne prouve donc rien dans un sens ni dans l'autre.`;
      case "tag_is_only_evidence":
        return "Un tag peut être recopié, obsolète ou faux. Sans confirmation structurelle, il ne suffit pas à conclure à un transcodage.";
      case "insufficient_signal":
        return "Le signal est silencieux ou trop faible pour fournir des indices spectraux fiables. Aucun verdict d'authenticité ne peut en être tiré.";
      case "integrity_mismatch":
        return "La somme de contrôle ne correspond pas à l'audio décodé. Le verdict de provenance est suspendu ; vérifiez ou ré-extrayez le fichier.";
      case "invalid_sample_rate":
        return "Fréquence d'échantillonnage invalide ; impossible d'évaluer le contenu spectral.";
      case "sharp_rolloff":
        return `Coupure franche (~${fmtNumber(i.steepness_db_per_khz, 0)} dB/kHz) autour de ${fmtNumber(i.edge_khz, 1)} kHz. Un encodeur, un filtre de mastering ou un ré-échantillonnage peuvent tous la produire : ce n'est pas une preuve de compression avec perte.`;
      case "no_encoder_lowpass":
        return `Aucune coupure franche et soutenue mesurée entre ${fmtNumber(i.scanned_from_khz, 0)} et ${fmtNumber(i.nyquist_khz, 1)} kHz. L'absence de coupure ne prouve pas l'absence de compression avec perte.`;
      case "transparent_encode_unseen":
        return "Une compression avec perte peut ne laisser aucun indice détectable par ces tests, notamment après traitement du signal. L'absence de grille confirmée ne garantit pas une source sans perte.";
      case "gradual_rolloff":
        return `Le contenu s'arrête autour de ${fmtNumber(i.cutoff_khz, 1)} kHz, mais la transition y est progressive (~${fmtNumber(i.steepness_db_per_khz, 0)} dB/kHz) plutôt que le mur quasi vertical que produit un codec. C'est compatible avec un master volontairement sombre, un report de vinyle ou de bande, ou un encodage avec perte dont cette méthode ne peut pas distinguer le filtre — pas de quoi trancher dans un sens ou dans l'autre.`;
      case "mdct_grid_aligned":
        return `Alignement MDCT compatible avec des blocs longs AAC (fenêtre ${i.window === "kaiser_bessel" ? "Kaiser–Bessel" : "sinus"}, décalage ${i.frame_offset}). Écart robuste : ${fmtNumber(i.z_score, 0)} au balayage, ${fmtNumber(i.confirmed_z_score, 0)} sur des trames distinctes de confirmation ; ${fmtNumber(i.zero_percent, 1)} % de coefficients quasi nuls contre ${fmtNumber(i.baseline_percent, 1)} % aux décalages témoins. Indice structurel fort, pas une preuve absolue ni une probabilité calibrée.`;
      case "mdct_grid_clear":
        return "Aucun alignement AAC confirmé pour les fenêtres et blocs longs testés. Cela n'exclut pas une source AAC : blocs courts, ré-échantillonnage, bruit ajouté ou transformations peuvent masquer la grille. Le MP3 n'est pas couvert par ce test.";
      case "declared_lossy_codec":
        return `Ce fichier est en ${i.codec.toUpperCase()}, un format avec perte. Il ne se fait pas passer pour autre chose : il n'y a donc pas de transcodage à détecter, et la question à laquelle ce verdict répond — un conteneur sans perte cache-t-il de l'audio avec perte — ne s'applique pas. Toutes les mesures ci-dessous décrivent malgré tout le fichier fidèlement, y compris le passe-bas et la grille de trames de son propre encodeur.`;
      case "content_above_cd_ceiling":
        return `Énergie mesurée au-dessus de ${fmtNumber(i.ceiling_khz, 2)} kHz : ${fmtNumber(i.level_db, 0)} dB par rapport à la bande de référence. Du bruit ajouté ou un traitement après transcodage peut recréer cette énergie : elle n'établit pas l'authenticité.`;
      case "decode_incomplete": {
        const quoi =
          i.channels_unequal
            ? "les canaux ont des longueurs différentes"
            : i.skipped_packets === 0
              ? "le flux a changé de format ou demandé un redémarrage et le décodage s'est arrêté là"
              : i.stopped_early
                ? `${i.skipped_packets} paquet(s) n'ont pas pu être décodés, puis le décodage a été interrompu par un changement de format ou une demande de redémarrage`
                : `${i.skipped_packets} paquet(s) n'ont pas pu être décodés et ont été ignorés`;
        return `Une partie de l'audio n'est jamais parvenue à l'analyse : ${quoi}. Toutes les mesures ci-dessous ne décrivent que la portion qui s'est décodée, donc aucun verdict sur le fichier entier ne peut être rendu. Réparez ou ré-extrayez le fichier, puis relancez l'analyse.`;
      }
      default: {
        const exhaustive: never = i;
        return exhaustive;
      }
    }
  },
  findings: {
    checksumMismatchTitle: "Somme de contrôle invalide",
    checksumMismatchDetail:
      "L'audio ne correspond pas à la somme de contrôle stockée dans le fichier. Il a été tronqué, modifié ou corrompu depuis sa création.",
    incompleteDecodeTitle: (s) =>
      s.channels_unequal
        ? "Canaux de longueurs différentes"
        : s.skipped_packets === 0
          ? "Décodage interrompu en cours de fichier"
          : s.stopped_early
            ? `${s.skipped_packets} paquet(s) ignoré(s), puis décodage interrompu`
            : s.skipped_packets > 1
              ? `${s.skipped_packets} paquets endommagés ignorés`
              : "1 paquet endommagé ignoré",
    incompleteDecodeDetail:
      "Une partie du fichier n'est jamais parvenue à l'analyse, ou les canaux n'ont pas la même longueur. Chaque mesure ci-dessous décrit l'audio qui a survécu, pas la piste entière, et le verdict de transcodage est suspendu pour cette raison.",
    bitDepthPaddingTitle: (declared, effective) => `Conteneur ${declared} bits contenant de l'audio ${effective} bits`,
    bitDepthPaddingDetail: (effective, activePct) =>
      `Sur les ${activePct} % d'échantillons non silencieux, tous tombent exactement sur la grille de quantification à ${effective} bits : la résolution supplémentaire ne porte aucune information mesurable. C'est compatible avec un simple rembourrage plutôt qu'avec un véritable remastering. À noter : un fichier correctement dithéré avant rembourrage échapperait à ce test.`,
    limitedBandwidthTitle: (declaredKHz, usedKHz) => `Bande limitée à ${usedKHz} kHz (fichier ${declaredKHz} kHz)`,
    limitedBandwidthDetail: (pct) =>
      `La limite mesurée représente ${pct} % de la bande disponible. Cela peut venir d'un filtrage natif ou d'un sur-échantillonnage ; cette mesure ne permet ni de retrouver la fréquence d'origine, ni de garantir une conversion sans perte.`,
    clippedRunsTitle: (runs, samples) =>
      `${runs} passage(s) aplati(s) au plein échelle (${samples} échantillons concernés)`,
    clippedRunsDetail:
      "Des échantillons consécutifs plaqués au plein échelle : c'est compatible avec un écrêtage, là où la forme d'onde a été aplatie plutôt que reproduite. Un échantillon isolé au plein échelle est un transitoire fort, pas un écrêtage — seuls les passages soutenus sont comptés ici. Le seuil suit la profondeur déclarée du fichier."
  },
  errors: {
    cannotOpen: (raw) => `Ce fichier n'a pas pu être ouvert. (${raw})`,
    unsupported: (raw) => `Format non pris en charge, ou fichier corrompu. (${raw})`,
    noAudio: (raw) => `Aucun audio décodable n'a été trouvé dans ce fichier. (${raw})`,
    playbackUnavailable: (raw) =>
      `Lecture indisponible : aucune sortie audio n'a pu être ouverte. L'analyse ci-dessous n'est pas affectée.${raw ? ` (${raw})` : ""}`,
    exportSucceeded: (path) => `Rapport enregistré dans ${path}`,
    exportFailed: (reason) => `Le rapport n'a pas pu être enregistré. ${reason}`
  },
  file: {
    bandwidthUsed: "Bande passante utilisée",
    bandwidthPhrase: (used, nyquist) => `${used} sur ${nyquist}`,
    bandwidthUnmeasured: "aucune limite mesurable"
  },
  spectrum: {
    title: "Spectre",
    bandwidth: "Bande passante",
    rolloffSteepness: "Pente de coupure",
    noEdgeFound: "aucune coupure détectée",
    noLimitMeasured: "aucune limite mesurable",
    steepnessValue: (db, hz) => `${db} dB/kHz à ${hz}`,
    note: "La bande passante indique où le contenu s'arrête, quand ce point est mesurable ; « aucune limite mesurable » signifie que le balayage n'a trouvé aucun point d'arrêt, ce qui n'est pas la même chose qu'un contenu qui monte jusqu'à Nyquist. Un filtre de mastering ou un ré-échantillonnage peut produire la même coupure franche qu'un codec : la pente seule ne détermine pas la provenance. Cliquez sur le spectrogramme pour déplacer la lecture à cet endroit."
  },
  loudness: {
    title: "Sonie",
    integratedLoudness: "Sonie intégrée",
    na: "n/d",
    unknownLayoutNote: "Positions des canaux inconnues : LUFS et plage de sonie non calculés. Les crêtes restent mesurées sur tous les canaux.",
    lufsTargetNote: "le repère marque -14 LUFS, cible courante des plateformes de streaming (une convention, pas une norme)",
    truePeak: "Crête réelle",
    clipWarnNote: "au-dessus du plein échelle — peut écrêter lors d'un ré-échantillonnage ou d'un ré-encodage en aval",
    headroomNote: "le repère marque la marge de -1 dBTP demandée par l'EBU R128",
    noOversamplingNote:
      "à cette fréquence d'échantillonnage, la bibliothèque n'applique aucun sur-échantillonnage : c'est un pic échantillonné, pas une crête inter-échantillon",
    loudnessRange: "Plage de sonie",
    peakRms: "Crête / RMS"
  },
  dynamics: {
    title: "Dynamique",
    dynamicRange: "Plage dynamique",
    drLabel: (dr) => (dr >= 14 ? "très élevée" : dr >= 12 ? "élevée" : dr >= 8 ? "moyenne" : "faible"),
    drNote: (label) => `${label} — échelle DR Pleasurize, celle qu'utilise la base de données loudness-war. Une mesure conventionnelle, pas une note de qualité.`,
    clippedRuns: "Passages aplatis",
    fullScaleNote: (samples) =>
      `${samples} échantillon(s) au plein échelle au total — un échantillon isolé est un transitoire fort, pas un écrêtage`,
    table: { ch: "Ch", peak: "Crête", rms: "RMS", crest: "Crest", dr: "DR", fullScale: "Pleine éch.", clipped: "Aplatis" },
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
    bandLevels: "Niveaux par bande (dB)",
    bandLevelsNote: "Niveau moyen de chaque bande, relatif à la plus forte du fichier. Une chute entre bandes voisines peut révéler un filtre, mais pas sa cause.",
    toNyquist: "Nyquist"
  },
  compare: {
    title: "Comparaison",
    action: "Comparer",
    add: "Comparer avec un autre fichier",
    exit: "Fermer la comparaison",
    metric: "Mesure",
    note: "Les lignes où les deux fichiers diffèrent sont mises en avant. Là où « mieux » a un sens sans ambiguïté — bande passante, plage dynamique, échantillons écrêtés — le côté qui mène est signalé. Ailleurs les deux valeurs sont simplement posées côte à côte : arbitrer entre un spectre plus large et un master plus fort demanderait une pondération que cet outil n'a aucune base pour établir.",
    loading: "Analyse du second fichier…",
    groupDeclared: "Ce que le fichier annonce",
    groupSpectrum: "Ce que le spectre montre",
    yes: "oui",
    no: "non",
    errorPrefix: "Comparaison impossible :",
    nowPlaying: (filename) => `Lecture : ${filename}`,
    leads: "valeur en tête pour cette mesure",
    evidence: "Indices"
  },
  disclaimer:
    "Nyquist rapporte ce qu'il peut mesurer et le dit clairement quand ce n'est pas suffisant. Le verdict de transcodage repose surtout sur la forme de la pente spectrale, qui ne voit pas un encodage transparent comme LAME V0 : sur un fichier à 44,1 kHz sans coupure détectable, « indéterminé » est le résultat honnête et attendu. Un résultat propre n'est jamais une preuve de provenance. La force des indices est une appréciation qualitative, pas une probabilité calibrée.",
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
    rawCutoff: (freq) => `coupure brute ~${freq}Hz`,
    quiet: "Faible",
    loud: "Fort",
    palette: "Palette du spectrogramme",
    paletteName: (n) =>
      n === "inferno" ? "Inferno" : n === "viridis" ? "Viridis" : n === "ice" ? "Glace" : "Monochrome"
  },
  mdct: {
    title: "Grille MDCT",
    detected: "Grille d'encodeur AAC détectée",
    clear: "Aucun alignement AAC confirmé",
    notAnalyzed: "Signal exploitable ou variation entre décalages insuffisants pour confirmer",
    offset: "Décalage",
    zeroed: "Quasi-zéros (confirmation)",
    baseline: "ailleurs",
    strength: "Écart",
    confirmation: "Confirmation",
    window: "Fenêtre",
    sineWindow: "Sinus",
    axisOffset: "décalage de trame (échantillons)",
    chartAria: "Profil du balayage de la grille MDCT",
    note: "Chaque colonne est un décalage de trame possible ; sa hauteur, la part de coefficients MDCT annulés à ce décalage. Un pic isolé peut révéler une grille AAC. Les fenêtres sinus et Kaiser–Bessel sont testées, puis l'alignement doit se confirmer sur des trames distinctes. Ce test structurel complète le spectre ; son absence ne garantit pas une source sans perte. Le MP3 n'est pas couvert : son banc de filtres hybride n'est pas une MDCT simple."
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
    progress: (completed, total) => `${completed}/${total} stages complete — stage durations vary`,
    text: "Decoding and analyzing…",
    hint: "Full-length FFT and loudness passes over every sample."
  },
  dragOverlay: "Drop to analyze",
  verdict: {
    probablyAuthentic: {
      label: "Probably authentic",
      blurb: "Positive evidence was measured that rules out a CD-rate lossy source."
    },
    probablyTranscoded: {
      label: "Probably transcoded",
      blurb: "This looks like lossy audio wrapped in a lossless container."
    },
    indeterminate: {
      label: "Inconclusive",
      blurb: "The available evidence cannot establish the source history. This is neither a guarantee of authenticity nor an accusation."
    },
    declaredLossy: {
      label: (codec) => `Lossy format (${codecName(codec)})`,
      blurb: "This file is in a lossy format and is not pretending otherwise, so there is nothing to see through. The measurements below still describe it in full."
    },
    confidence: "evidence strength",
    strength: { weak: "weak", moderate: "moderate", strong: "strong" }
  },
  // The backend authors these in English already. Handing its own prose straight back keeps
  // the app, `nyquist-cli` and an exported report word-for-word identical, and leaves the
  // wording with a single owner: `IndicatorDetail::english` in transcode_detect.rs.
  indicator: (i) => i.message,
  findings: {
    checksumMismatchTitle: "Checksum mismatch",
    checksumMismatchDetail:
      "The audio does not match the checksum stored inside the file. It has been truncated, edited, or corrupted since it was created.",
    incompleteDecodeTitle: (s) =>
      s.channels_unequal
        ? "Channels of unequal length"
        : s.skipped_packets === 0
          ? "Decoding stopped part-way through the file"
          : s.stopped_early
            ? `${s.skipped_packets} packet${s.skipped_packets > 1 ? "s" : ""} skipped, then decoding stopped`
            : `${s.skipped_packets} damaged packet${s.skipped_packets > 1 ? "s" : ""} skipped`,
    incompleteDecodeDetail:
      "Part of the file never reached the analysis, or its channels came out different lengths. Every measurement below describes the audio that survived, not the whole track, and the transcode verdict is withheld for that reason.",
    bitDepthPaddingTitle: (declared, effective) => `${declared}-bit container holding ${effective}-bit audio`,
    bitDepthPaddingDetail: (effective, activePct) =>
      `Across the ${activePct}% of samples that are not silent, every one lands exactly on the ${effective}-bit quantization grid, so the extra depth carries no measurable information. That is consistent with padding rather than a genuine remaster. Note that a file properly dithered before padding would escape this test.`,
    limitedBandwidthTitle: (declaredKHz, usedKHz) => `Bandwidth limited to ${usedKHz} kHz (${declaredKHz} kHz file)`,
    limitedBandwidthDetail: (pct) =>
      `The measured limit occupies ${pct}% of the available band. Native filtering and upsampling can both cause this; the measurement cannot recover the original rate or guarantee lossless conversion.`,
    clippedRunsTitle: (runs, samples) => `${runs} flattened run(s) at full scale (${samples} samples involved)`,
    clippedRunsDetail:
      "Consecutive samples pinned at full scale, which is consistent with clipping — the waveform flattened rather than reproduced. A lone full-scale sample is a loud transient, not clipping, so only sustained runs are counted here. The threshold follows the file's declared bit depth."
  },
  errors: {
    cannotOpen: (raw) => `This file could not be opened. (${raw})`,
    unsupported: (raw) => `This format is not supported, or the file is corrupt. (${raw})`,
    noAudio: (raw) => `No decodable audio was found in this file. (${raw})`,
    playbackUnavailable: (raw) =>
      `Playback is unavailable: no audio output could be opened. The analysis below is unaffected.${raw ? ` (${raw})` : ""}`,
    exportSucceeded: (path) => `Report saved to ${path}`,
    exportFailed: (reason) => `The report could not be saved. ${reason}`
  },
  file: {
    bandwidthUsed: "Bandwidth used",
    bandwidthPhrase: (used, nyquist) => `${used} of ${nyquist}`,
    bandwidthUnmeasured: "no measurable limit"
  },
  spectrum: {
    title: "Spectrum",
    bandwidth: "Bandwidth",
    rolloffSteepness: "Rolloff steepness",
    noEdgeFound: "no edge found",
    noLimitMeasured: "no measurable limit",
    steepnessValue: (db, hz) => `${db} dB/kHz @ ${hz}`,
    note: "Bandwidth is where content stops, when that point is measurable; \"no measurable limit\" means the sweep found no stopping point, which is not the same as content running all the way to Nyquist. Mastering filters and resampling can produce the same sharp edge as a codec: steepness alone cannot determine provenance. Click the spectrogram to jump playback there."
  },
  loudness: {
    title: "Loudness",
    integratedLoudness: "Integrated loudness",
    na: "n/a",
    unknownLayoutNote: "Unknown channel positions: LUFS and loudness range withheld. Peaks still include every channel.",
    lufsTargetNote: "tick marks -14 LUFS, a common streaming platform target (a convention, not a standard)",
    truePeak: "True peak",
    clipWarnNote: "above full scale — may clip when resampled or re-encoded downstream",
    headroomNote: "tick marks the -1 dBTP headroom EBU R128 asks for",
    noOversamplingNote:
      "at this sample rate the library applies no oversampling, so this is a sampled peak rather than an intersample one",
    loudnessRange: "Loudness range",
    peakRms: "Peak / RMS"
  },
  dynamics: {
    title: "Dynamics",
    dynamicRange: "Dynamic range",
    drLabel: (dr) => (dr >= 14 ? "very high" : dr >= 12 ? "high" : dr >= 8 ? "medium" : "low"),
    drNote: (label) => `${label} — Pleasurize DR scale, the one the loudness-war database uses. A conventional measurement, not a quality grade.`,
    clippedRuns: "Flattened runs",
    fullScaleNote: (samples) =>
      `${samples} sample(s) at full scale in total — a lone one is a loud transient, not clipping`,
    table: { ch: "Ch", peak: "Peak", rms: "RMS", crest: "Crest", dr: "DR", fullScale: "Full scale", clipped: "Flattened" },
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
    bandLevels: "Band levels (dB)",
    bandLevelsNote: "Average level of each band, relative to the loudest in the file. This is the spectral shape the verdict is drawn from: an encoder wall shows up as an abrupt fall between neighbouring bands, a dark master as a steady slope.",
    toNyquist: "Nyquist"
  },
  compare: {
    title: "Comparison",
    action: "Compare",
    add: "Compare with another file",
    exit: "Close comparison",
    metric: "Measurement",
    note: "Rows where the two files disagree are brought forward. Where \"better\" is unambiguous — bandwidth, dynamic range, clipped samples — the leading side is marked. Everywhere else the two values are simply placed side by side: choosing between a wider spectrum and a louder master would need a weighting this tool has no basis to set.",
    loading: "Analyzing the second file…",
    groupDeclared: "What the file claims",
    groupSpectrum: "What the spectrum shows",
    yes: "yes",
    no: "no",
    errorPrefix: "Cannot compare:",
    nowPlaying: (filename) => `Playing: ${filename}`,
    leads: "leading value for this measurement",
    evidence: "Evidence"
  },
  disclaimer:
    "Nyquist reports what it can measure and says so when that is not enough. The transcode verdict rests mainly on the shape of the spectral rolloff, which cannot see a transparent encode such as LAME V0: on a 44.1 kHz file with no detectable cutoff, \"indeterminate\" is the honest and expected result. A clean result is never proof of provenance. Evidence strength is a qualitative reading, not a calibrated probability.",
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
    rawCutoff: (freq) => `raw cutoff ~${freq}Hz`,
    quiet: "Quiet",
    loud: "Loud",
    palette: "Spectrogram palette",
    paletteName: (n) =>
      n === "inferno" ? "Inferno" : n === "viridis" ? "Viridis" : n === "ice" ? "Ice" : "Monochrome"
  },
  mdct: {
    title: "MDCT grid",
    detected: "AAC encoder grid detected",
    clear: "No confirmed AAC alignment",
    notAnalyzed: "Insufficient usable signal or offset variation for confirmation",
    offset: "Offset",
    zeroed: "Near-zeros (confirmation)",
    baseline: "elsewhere",
    strength: "Margin",
    confirmation: "Confirmation",
    window: "Window",
    sineWindow: "Sine",
    axisOffset: "frame offset (samples)",
    chartAria: "MDCT grid sweep profile",
    note: "Each column is a candidate frame offset; its height is the share of MDCT coefficients reading as zeroed there. An isolated peak can reveal an AAC grid. Sine and Kaiser–Bessel windows are tested, then the offset must hold on disjoint confirmation frames. This structural test complements the spectrum; a negative search does not establish a lossless source. MP3 is not covered: its hybrid filterbank is not a plain MDCT."
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
