/** Minimal stroke-style icon set (feather-icon convention). Inner SVG markup only —
 * wrap with `<svg viewBox="0 0 24 24">` at the call site. No icon library dependency. */
export const icons = {
  disc: `<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="2.5"/>`,
  activity: `<path d="M3 12h4l2 7 4-14 2 7h6"/>`,
  layers: `<path d="M12 4l8 4-8 4-8-4 8-4z"/><path d="M4 12l8 4 8-4"/><path d="M4 16l8 4 8-4"/>`,
  stereo: `<circle cx="8" cy="12" r="4"/><circle cx="16" cy="12" r="4"/>`,
  clock: `<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3.5 2"/>`,
  file: `<path d="M7 3h7l5 5v13a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z"/><path d="M14 3v5h5"/>`,
  gauge: `<path d="M4 15a8 8 0 0 1 16 0"/><path d="M12 15l4-5"/><circle cx="12" cy="15" r="1"/>`,
  arrowsVertical: `<path d="M12 3v18"/><path d="M7 8l5-5 5 5"/><path d="M7 16l5 5 5-5"/>`,
  peak: `<path d="M3 18l6-10 4 6 3-4 5 8z"/>`,
  speaker: `<path d="M4 9v6h4l5 4V5L8 9H4z"/><path d="M17 9a4 4 0 0 1 0 6"/>`,
  triangle: `<path d="M12 4l9 16H3z"/>`,
  clip: `<path d="M3 17h4v-6h4v6h4v-10h4"/>`,
  funnel: `<path d="M4 4h16l-6 8v6l-4 2v-8z"/>`,
  hash: `<path d="M4 9h16M4 15h16M9 4l-2 16M17 4l-2 16"/>`,
  upload: `<path d="M12 16V4M7 9l5-5 5 5"/><path d="M4 16v3a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-3"/>`,
  checkCircle: `<circle cx="12" cy="12" r="9"/><path d="M8 12.5l2.5 2.5 5-6"/>`,
  alertCircle: `<circle cx="12" cy="12" r="9"/><path d="M12 7.5v5.5"/><path d="M12 16.2v.01"/>`,
  helpCircle: `<circle cx="12" cy="12" r="9"/><path d="M9.3 9.2a2.7 2.7 0 0 1 5.2.9c0 1.7-2.5 2.1-2.5 3.7"/><path d="M12 16.7v.01"/>`,
  play: `<path d="M7 4.5v15l13-7.5z" fill="currentColor" stroke="none"/>`,
  pause: `<path d="M8 5v14M16 5v14" stroke-width="2.4"/>`,
  shield: `<path d="M12 3l7 3v5.5c0 4.6-3 8.3-7 9.5-4-1.2-7-4.9-7-9.5V6l7-3z"/>`,
  download: `<path d="M12 4v12M7 11l5 5 5-5"/><path d="M4 19h16"/>`,
  tag: `<path d="M20 12l-8 8-9-9V4h7l10 10z"/><path d="M8 8v.01"/>`,
  waveform: `<path d="M2 12h3l2-6 3 13 3-9 2 4 3-3h4"/>`,
  ruler: `<path d="M3 8h18v8H3z"/><path d="M7 8v3M11 8v5M15 8v3M19 8v5"/>`,
  sun: `<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>`,
  moon: `<path d="M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5z"/>`,
  alertTriangle: `<path d="M12 4l9 16H3z"/><path d="M12 10v4"/><path d="M12 17.2v.01"/>`,
  refresh: `<path d="M20 12a8 8 0 1 1-2.3-5.6"/><path d="M20 4v4h-4"/>`,
  volumeMute: `<path d="M4 9v6h4l5 4V5L8 9H4z"/><path d="M16 9l6 6M22 9l-6 6"/>`
} as const;

export type IconName = keyof typeof icons;
