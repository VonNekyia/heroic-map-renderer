---
title: Drittlizenzen
description: Was jeder Weitergabe des Binärs beiliegt - LICENSE, NOTICE, THIRD-PARTY-NOTICES und COPYRIGHT-library.html -, wie renderer/drittlizenzen.py die Hinweise auf die Lizenzen der 110 Crates aus Cargo.lock erzeugt, welche Lizenz es bei einer Wahl nimmt und was die CI daran prüft.
code:
  - renderer/drittlizenzen.py
  - renderer/deny.toml
  - LICENSE
  - NOTICE
---

# Drittlizenzen

Jede Weitergabe des Binärs, etwa ein Release oder das Plugin, legt vier
Dateien bei: `LICENSE` und `NOTICE` aus dem Repo, `THIRD-PARTY-NOTICES` und
`COPYRIGHT-library.html`. Die letzten zwei erzeugt
[`renderer/drittlizenzen.py`](../../renderer/drittlizenzen.py) für genau den
Stand, der gebaut wird. Warum Apache-2.0 und welche Lizenzen Abhängigkeiten
haben dürfen: [0078](../entscheidungen/0078-apache-2-0.md).

## Erzeugen

```bash
python renderer/drittlizenzen.py <zielordner>
```

- **Mit demselben `rustc` wie der Build:** `COPYRIGHT-library.html` kommt
  aus seiner Toolchain (`rustc --print sysroot`, `share/doc/rust`). Sie
  nennt die Lizenzen der Standardbibliothek und ihrer Abhängigkeiten und
  ändert sich mit der Rust-Version. Deshalb liegt keine der zwei Dateien im
  Repo.
- **Reproduzierbar:** Gleiches `Cargo.lock` und gleiche Toolchain geben
  dieselben Bytes.
- **Die Crates:** jede normale Abhängigkeit ab der eigenen Crate, für
  `x86_64-pc-windows-msvc` und `x86_64-unknown-linux-gnu` zusammen (`ZIELE`),
  aus `cargo metadata`. Build- und Dev-Abhängigkeiten und proc-macro-Crates
  landen nicht im Binär und fehlen. Stand 05.10.: 110 Crates.

## Welche Lizenz gilt

- **Bei einer Wahl** („OR“, auch die alte Schreibweise mit „/“) nimmt das
  Skript die erste nach `VORZUG`: Apache-2.0, sonst MIT, Zlib, ISC, BSD
  und die übrigen. Bei „AND“ gelten alle. Stand 05.10. gilt für 93 Crates
  Apache-2.0, dafür reicht ein gemeinsamer Text.
- **Je übrige Lizenz** der Text aus der Crate, mit ihrem Copyright: die
  Datei, deren Name die Lizenz nennt (`LICENSE-MIT`), sonst die einzige
  Lizenzdatei. Dazu jede `NOTICE` und `PATENTS` der Crate.
- **Mitgelieferte C-Quellen** (`MITGELIEFERT`): libwebp in `libwebp-sys`,
  seit [0092](../entscheidungen/0092-kompakt-ohne-vorhersage.md) aus der Kopie
  unter `renderer/vendor/libwebp-sys`, siehe
  [libwebp mit Patch](libwebp-mit-patch.md)
  (`vendor/COPYING`, BSD-3-Clause, und `vendor/PATENTS`), mimalloc in
  `libmimalloc-sys` (`c_src/mimalloc/*/LICENSE`, MIT). Dazu bei `ring` die
  Übersicht `LICENSE` und der Code aus once_cell
  (`src/polyfill/once_cell/LICENSE-MIT`).
- **Texte unter fremdem Namen** (`TEXT_DER_LIZENZ`): `ring` legt seinen
  ISC-Text als `LICENSE-other-bits` ab, neben weiteren Lizenzdateien; der
  Name verrät die Lizenz nicht.
- **Ohne Text:** Bringt eine Kiste mit MIT keinen Text mit, nimmt das
  Skript den Mustertext der MIT-Lizenz aus der Toolchain mit den Autoren aus
  `Cargo.toml`. So war es bei `libwebp-sys` von crates.io; die Kopie unter
  `renderer/vendor` bringt seit 0092 `LICENSE-MIT` mit. Fehlt der Text einer anderen Lizenz, bricht es
  ab.
- **Kodierung:** Die Ausgabe von `cargo metadata` und `rustc` liest das
  Skript als UTF-8. Unter Windows nähme Python sonst cp1252, und Namen
  ausserhalb von ASCII kämen verdorben ins Paket (#200). Steht ein Autor aus
  `cargo metadata` nicht wörtlich in der `Cargo.toml` der Crate, bricht es
  ab.
- **Gleiche Texte** stehen einmal, mit allen Crates, für die sie gelten.

## In der CI

Der Job „Dependencies“ prüft mit `cargo deny check` die Lizenzen aller
Crates gegen `renderer/deny.toml`, die eigene eingeschlossen. Danach lässt
er das Skript laufen: Bringt eine neue Crate keinen Text mit, fällt der
Job, siehe [CI](ci.md). Der Job „Rust“ lässt es zusätzlich unter Windows
laufen, wo das Windows-Paket entsteht und die Kodierung zählt.
`selbsttest` prüft dabei die Wahl aus einem SPDX-Ausdruck an fünf Fällen.
