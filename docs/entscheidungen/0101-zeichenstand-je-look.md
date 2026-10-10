---
title: "0101: Ein Zeichenstand je Look"
description: Warum der Zeichenstand je Wert von look in map.json gilt statt für alle Bäume, warum der Abdruck für map und cinematic dabei gleich bleibt, wie die Goldbilder je Look den Zeichenstand absichern, was alle Looks teilen und darum jeden hebt, was voller-lauf.sh und stand.bin dafür brauchen und welche Wege verworfen sind; löst 0098 in diesem Punkt ab.
status: gilt
date: 2026-10-10
code:
  - renderer/src/render/stand.rs
  - renderer/src/cli.rs
  - .github/voller-lauf.sh
---

# 0101: Ein Zeichenstand je Look

Löst in [0098](0098-der-zeichenstand-statt-des-builds.md) den einen
Zeichenstand für alle Bäume ab und `GOLDBILDER` über alle Goldbilder. Der
Rest von 0098 gilt.

## Anlass

#257 ändert nur, wie `--flat` zeichnet. Mit einem Zeichenstand für alle
Bäume verlangte das nächste Release trotzdem für jede Karte und jedes
Cinematic einen vollen Lauf, auf grossen Welten Stunden für nichts. Nach
Regel 26 wiegt die Renderzeit, live mehr als initial.

## Entscheidung

- **Je Look ein Zeichenstand:** `ZEICHENSTAND_MAP`,
  `ZEICHENSTAND_CINEMATIC` und `ZEICHENSTAND_FLAT` in
  `renderer/src/render/stand.rs`, je Wert von `look` in `map.json`. Ab
  diesem Stand: `map` 3, `cinematic` 3, `flat` 4.
- **Der Abdruck** des Renderers für einen Baum ist FNV-1a über den
  Zeichenstand seines Looks und die eingebauten Tabellen, gerechnet wie bis
  hier. Den Look selbst nimmt er nicht auf: So ist der Abdruck von `map` und
  `cinematic` bitgleich mit dem von v0.7.0, Zeichenstand 3 mit denselben
  Tabellen (`abdruck_von_karte_und_cinematic_wie_in_v0_7_0`). Ihr Stand gilt
  weiter; nur Bäume mit `look` `flat` rendern einmal neu.
- **Welcher Look:** Die CLI nimmt den Look des Baums wie für `map.json`
  (`look_name` in `renderer/src/cli.rs`): `--cinematic`, sonst `--flat`,
  sonst die Karte. Ein Baum hat genau einen Look, er steckt auch in seinem
  Namen.
- **`stand.bin`** behält sein Format; `ZEICHENSTAND_1`, `ALTE_BUILDS` und
  `KODIERSTAND` bleiben, wie sie sind.
- **Goldbilder je Look:** `GOLDBILDER_JE_LOOK` ordnet jedes Goldbild
  ausdrücklich einem Look zu, `metatile-cinematic` zu `cinematic`,
  `metatile-flat` zu `flat`, die übrigen zu `map`. `GOLDBILDER` hält je Look
  FNV-1a über seine Goldbilder, und `zeichenstand_folgt_den_goldbildern`
  nennt den Look, der zu heben ist. `jedes_goldbild_hat_einen_look` fällt
  bei einem Goldbild, das in der Liste fehlt, statt es still einem Look
  zuzuschlagen.
- **`voller-lauf.sh`** liest den Zeichenstand je Look am letzten Tag und am
  neuen. Ein Tag mit nur einem `ZEICHENSTAND` gilt für alle Looks, einer
  ohne für Zeichenstand 1. Ändert sich eine Tabelle, braucht jeder Baum
  einen vollen Lauf. Sonst nennt die Ausgabe die Looks, die einen brauchen,
  und die, die keinen brauchen; der Wortlaut steht in
  [Plugin](../plugin.md), „Was das Plugin vom Renderer nutzt“.

### Was die Looks teilen

Wer ändert, was mehrere Looks teilen, hebt jeden davon; im Zweifel jeden
Look (Regel 27).

| Teil | Looks |
|---|---|
| die eingebauten Tabellen unter `renderer/src/assets/` | alle, schon über den Abdruck |
| der Abdruck eines Chunks in `stand.bin` | alle |
| die Höhen, `heights/` und was `map.json` darüber sagt | alle |
| Deckung, Kandidaten und Fassungen | alle |
| Modelle, Sprites, Rasterizer, Biomfarben und Wasser | alle |
| die weiche Beleuchtung | alle |
| die Ausbreitung des Lichts | `map`, `cinematic` |
| HDR, Sonne, Bloom und das Verfahren des Looks | `cinematic`; seine Werte schützt zusätzlich `lookHash` |
| Relief, Licht je Spalte samt dem Schritt von der Seite, Bänder | `flat` |

## Kosten nach Regel 26

- **Live und initial:** Kein Lauf wird langsamer; der Abdruck ist derselbe
  Hash. Gespart wird der volle Lauf jedes Baums, dessen Look sich nicht
  ändert: Mit dem nächsten Release rendern nur Bäume mit `look` `flat`
  neu, auf der ganzen Testwelt rund 45 s, statt auch jeder Karte und jedes
  Cinematic.
- **Arbeitsspeicher und Platz:** gleich.

## Verworfene Alternativen

- **Ein Zeichenstand für alle,** wie in 0098: Jede Änderung an einem Look
  verlangte von jedem Baum einen vollen Lauf.
- **Der Look im Abdruck:** sauberer gegen Verwechslung, aber jeder Stand
  von heute würde ungültig, und jeder Baum renderte einmal neu.
- **Ein Zeichenstand je Baum oder Kamera:** Alle Kameras zeichnen über
  denselben Weg; die Liste würde lang, und das Review müsste je Kamera
  entscheiden.
- **Goldbilder nach ihrem Namen zuordnen:** Ein neues Goldbild ohne
  `-flat` oder `-cinematic` fiele still zu `map`.

## Folgen

- **Code:** die Konstanten und `GOLDBILDER` je Look in
  `renderer/src/render/stand.rs`, der Look des Baums beim Abdruck in
  `renderer/src/cli.rs`, `.github/voller-lauf.sh`.
- **Doku:** [Updates](../benutzung/updates.md) mit der Tabelle der
  Zeichenstände je Look, [Plugin](../plugin.md) mit dem Wortlaut der
  Notizen, Regel 27 in `AGENTS.md`, die Skills `doku-pflegen` und
  `goldbild-erneuern`.
- **Plugin:** Die Zeile `Error:` aus `stand_fuer_update` bleibt Wort für
  Wort; sie kommt nur noch für Bäume, deren Look anders zeichnet. Ob
  `/heroicmap render` einzelne Bäume annimmt, entscheidet das Plugin.
