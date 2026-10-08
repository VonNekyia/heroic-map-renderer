---
title: libwebp mit Patch
description: Warum libwebp-sys als eigene Kopie mit einem Patch an libwebp im Repository liegt, was der Patch ändert und was nicht, wie die Kopie eingebunden ist und was bei einem neuen libwebp-sys zu tun ist.
code:
  - renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c
  - renderer/vendor/libwebp-sys/rustfmt.toml
  - renderer/Cargo.toml
  - renderer/deny.toml
  - renderer/src/render/tiles.rs
---

# libwebp mit Patch

Für `--compact` soll libwebp bei Kacheln ohne Palette die räumliche
Vorhersage weglassen. Über seine API lässt es sich das nicht sagen. Deshalb
liegt `libwebp-sys` 0.14.4 mit libwebp 1.6.0 als Kopie unter
`renderer/vendor/libwebp-sys` und kommt über `[patch.crates-io]` in
[`renderer/Cargo.toml`](../../renderer/Cargo.toml) in den Bau. Die Kopie
gleicht der Kiste von crates.io bis auf den Patch in libwebp und zwei
Zeilen im Bauskript. Warum:
[0092](../entscheidungen/0092-kompakt-ohne-vorhersage.md).

## Der Patch

- **Wo:** `EncoderAnalyze` in
  [`vendor/src/enc/vp8l_enc.c`](../../renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c),
  markiert mit dem Kommentar `heroic-map-renderer:`.
- **Was:** Ist `image_hint` `WEBP_HINT_GRAPH` und hat das Bild keine
  Palette, nimmt libwebp statt seiner Schätzung `kSubGreen`, nur
  „subtract green“. Ohne Vorhersage fällt auch „Farbe quer“ weg.
- **Wann es wirkt:**
  - nur bei method 1 bis 6, wo libwebp eine Transformation schätzt;
  - nicht bei method 0, das einen eigenen Weg nimmt, und nicht bei
    method 6 mit quality 100, die alle Transformationen probiert.

  Bisher las libwebp `WEBP_HINT_GRAPH` im verlustfreien Weg nur für die
  Grösse eines Puffers. Ohne diesen Hinweis packt die Kopie Byte für Byte
  wie das Original.
- **Geprüft:**
  - `kompakt_ohne_vorhersage` in
    [`renderer/tests/tiles.rs`](../../renderer/tests/tiles.rs) liest die
    Transformationen aus dem Bitstrom: kompakt nur „subtract green“,
    schnell mit Vorhersage.
  - Unter Windows gibt libwebp mit SSE4.1 und AVX2, mit SSE2 allein und
    ohne SIMD dieselben Bytes, auch mit dem Patch. Gemessen in
    [2026-10-08, Kompakt packen](../messungen/2026-10-08-kompakt.md).

## Was zur Kopie gehört

- **Dateien:** wie die Kiste sie mitbringt: `Cargo.toml`, `build.rs`,
  `wrap.h`, `README.md`, `src/` und unter `vendor/` den Quelltext von
  libwebp mit `COPYING` und `PATENTS`. Dazu `LICENSE-MIT`: Die Kiste nennt
  MIT, bringt den Text aber nicht mit.
- **Ein Übergang:** Bietet libwebp die Einstellung selbst an, fällt die
  Kopie weg, und `[patch.crates-io]` mit ihr.
- **`build.rs`** meldet Cargo `vendor/src` und `vendor/sharpyuv` zum
  Neubau, ebenfalls mit `heroic-map-renderer:` markiert. Sonst baute Cargo
  libwebp nach einer Änderung am C-Quelltext nicht neu: `cc` meldet
  `rerun-if-env-changed`, und dann zählen Dateien nur noch, wenn das Skript
  sie nennt.
- **`rustfmt.toml`** mit `disable_all_formatting = true`: `cargo fmt --all`
  formatiert auch lokale Pfad-Abhängigkeiten, und die CI prüft das, siehe
  [CI](ci.md). Die Kopie bleibt, wie sie ist.
- **Lizenzen:** Die Klarstellung `MIT AND BSD-3-Clause` in
  [`renderer/deny.toml`](../../renderer/deny.toml) gilt für die Kopie wie
  für das Original; `vendor/COPYING` ist unverändert. Die Hinweise im
  Paket erzeugt `drittlizenzen.py` aus der Kopie, mit `LICENSE-MIT`, siehe
  [Drittlizenzen](drittlizenzen.md).
- **Clippy** prüft nur das eigene Paket, nicht die Kopie.

## Ein neues libwebp-sys

1. Die neue Kiste aus dem Cargo-Cache in `renderer/vendor/libwebp-sys`
   kopieren, mit denselben Dateien wie oben, dazu wieder `rustfmt.toml` und
   `LICENSE-MIT` mit den Autoren aus ihrer `Cargo.toml`.
2. Den Patch an `EncoderAnalyze` und die zwei Zeilen in `build.rs` neu
   setzen. Steht dort nicht mehr dieselbe Wahl einer Transformation, erst
   die neue Stelle verstehen.
3. `kompakt_ohne_vorhersage` und die Suite laufen lassen. Die Prüfung über
   die SIMD-Wege wiederholen, siehe die Messung oben.
4. Den Hash der Klarstellung in `renderer/deny.toml` prüfen, falls sich
   `vendor/COPYING` geändert hat.
5. Ändern sich die Bytes, hat jede Kachel nach dem nächsten vollen Lauf ein
   neues ETag, wie bei [0090](../entscheidungen/0090-webp-mit-quality-75.md).
