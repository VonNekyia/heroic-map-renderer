/**
 * Der Skin aus dem Schalter `SKIN` beim Build; ohne Default-Export
 * `undefined`. Siehe docs/frontend.md, „Skins“.
 */
declare module 'virtual:skin' {
  const skin: import('./skin-api').Skin | undefined;
  export default skin;
}

/** Ist beim Build ein Skin gesetzt? Ohne fällt sein Import aus dem Bündel. */
declare const __SKIN__: boolean;

/** Die Texte für den Skin aus der Build-Konfiguration. */
declare const __SKIN_TEXTE__: Readonly<Record<string, string>>;
