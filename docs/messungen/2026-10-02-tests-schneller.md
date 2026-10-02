---
title: Tests schneller
description: Was die schlanke Auswahl in `kameras()`, parallele Tests, Testbauten mit Optimierung und das Profil `mutation` an Bauen und Testen ändern, lokal im Debug-Build und in der CI; dazu, warum die Reihe zu den Testbauten mit Optimierung nicht zählt.
date: 2026-10-02
commits: [666cd46, 2c8ee57, dad606b, 7e66363]
code:
  - renderer/tests/metatile.rs
  - renderer/tests/cli.rs
  - renderer/Cargo.toml
---

# Tests schneller

Mit der schlanken Auswahl in `kameras()` braucht die ganze Suite im
Debug-Build lokal 178 statt 268 s, −33 %. Fast die ganze Zeit kostet
`schneller_weg_gleicht_der_referenz`, er braucht 153 statt 240 s. Rechnen
er und `native_stufen_wie_der_weg_je_stufe` parallel, sind es in einem
einzelnen Lauf 74 s, mit Testbauten mit Optimierung 20 s. Eine Mutation baut
mit dem Profil `mutation` in 11 statt 86 s. In der CI sinkt der Schritt
Tests auf ubuntu von 311 auf 75 s, auf windows von 432 auf 195 s, der
längste Job von 502 auf 318 s. Die Testbauten mit Optimierung hat die Reihe
nicht gemessen, weil die Schalter dort nicht wirkten; sie stammen aus der CI
und einzelnen Läufen.

## Aufbau

- Stände, je aus eigenem Worktree mit eigenem Zielverzeichnis:
  - **A:** `666cd46`, master nach #87: jede Kamera aus `kameras()` läuft
    auch aus einer anderen Richtung.
  - **B0:** `2c8ee57`: aus einer anderen Richtung nur eine Auswahl je Art,
    siehe [Tests](../entwicklung/tests.md), „Kameras“; dazu das Profil
    `mutation`.
  - **B1:** B0 mit `profile.dev.package."*".opt-level=2`, **B2:** dazu
    `profile.test.opt-level=1`, beide per `--config`. Sie zählen nicht,
    siehe „Ablauf“.
- Befehle aus `renderer/`, Debug-Build:

```bash
cargo nextest run --no-run --build-jobs 12
cargo nextest run --build-jobs 12 --test-threads 12 --no-fail-fast
```

- **Kalt bauen:** der erste Befehl in ein leeres Zielverzeichnis.
- **Neu bauen:** `renderer/src/render/metatile.rs` berühren, dann der erste
  Befehl.
- **Testen:** der zweite Befehl, die ganze Suite, 586 Tests.
- **Mutation:** in B eine Zeile in `ChunkCache::zelle`
  (`renderer/src/render/metatile.rs`) falsch: die Spalte der Maske aus den
  Koordinaten der Welt statt aus denen im Blick. Gebaut mit `--release`
  gegen `--cargo-profile mutation`, getestet nur `-E 'binary(richtung)'`;
  ein Test fällt, wie er soll. Nach jeder Runde zurückgesetzt und ohne Zeit
  neu gebaut.

## Ablauf

- Messskript am 02.10. ab 17:00 am Stück, mit Sperrdatei wie im Skill
  [`messung-protokollieren`](../../skills/messung-protokollieren/SKILL.md):
  - je Stand drei kalte Bauten, B0, B1, B2, dann umgekehrt, dann wieder;
  - drei Runden je Stand neu bauen und testen, A, B0, B1, B2, dann
    umgekehrt, dann wieder;
  - drei Runden der Mutation, release und Profil im Wechsel.
- Nach jedem Lauf 15 s Pause. Die Zeit ist die Wanduhr um den Aufruf von
  cargo; beim Testen dazu die Summe aus der Ausgabe von nextest und die
  Zeile von `schneller_weg_gleicht_der_referenz`.
- **Last:** vor jedem Lauf gemittelt über 20 s. 10 von 39 Läufen lagen
  über 10 %, bis 13,25 %. Das Skript prüfte die Last nur vor der Reihe.
  Unten zählen nur Läufe unter 10 %.
- **B1 und B2 zählen nicht.** Das Skript setzte `--config` vor den
  Unterbefehl, `cargo --config … nextest run`. Dort lässt cargo es fallen:
  Die Logs zeigen auch für B2 `` Finished `test` profile [unoptimized +
  debuginfo] ``. B1 und B2 haben also B0 gemessen, und ihre Zeiten liegen
  bei denen von B0. Hinter `nextest run` wirkt es, mit `--cargo-verbose`
  nachgesehen: die Abhängigkeiten mit `opt-level` 2, das Crate und die
  Tests mit 1.
- **Eine zweite Reihe** mit B0, B1 und B2 sollte um 19:45 beginnen. Bis
  20:33 lag die Last bei 15 bis 26 %, sie brach ab, ohne zu messen. Statt
  einer weiteren Reihe misst die CI der PR die Testbauten mit Optimierung.

## Ergebnis

Debug-Build, je Lauf, unter 10 % Last:

| | A | B0 |
|---|---|---|
| ganze Suite, nextest | 264,8 / 268,2 / 274,5 s | 173,6 / 178,4 / 194,7 s |
| `schneller_weg_gleicht_der_referenz` | 238,8 / 240,0 / 243,0 s | 148,7 / 152,8 / 153,0 s |
| neu bauen | 10,2 / 11,2 / 21,6 s | 10,1 / 11,9 / 31,4 s |
| kalt bauen | – | 65,3 / 66,9 s |

- **Die Suite:** Median 268 → 178 s, −33 %.
  `schneller_weg_gleicht_der_referenz` 240 → 153 s, −36 %.
- **Neu bauen:** gleich. Der längere Lauf ist je Stand der erste nach dem
  kalten Bau.
- **Kalt bauen:** A nicht gemessen. Der dritte Lauf von B0 lag mit 63,0 s
  bei 10,15 % Last.

Eine Mutation:

| | `--release` | `--cargo-profile mutation` |
|---|---|---|
| bauen | 86,1 s | 10,3 / 11,0 s |
| testen | 1,1 s | 1,1 / 1,3 s |

- Rund achtmal schneller gebaut, das Testen ist gleich.
- Nicht gezählt: release in Runde 2 und 3, 112,5 und 109,7 s bei 12,95 und
  11,35 % Last; das Profil in Runde 3, 10,9 s bei 11,25 %.

## Einzelne Läufe danach

Je ein Lauf der Suite, keine Reihe und ohne Prüfung der Last, am Stand nach
dem Merge von master mit #91, 593 Tests. Die Zeit je Test steht in der
Ausgabe von nextest.

| Stand | Suite | `schneller_weg_…` | `native_stufen_…` |
|---|---|---|---|
| schlank, `73be560` | 203 s | 175 s | 66 s |
| `schneller_weg_…` parallel | 85 s | 38 s | 72 s |
| dazu die Exporte nebeneinander, `dad606b` | 74 s | 40 s | 61 s |
| dazu Testbauten mit Optimierung, `7e66363` | 20 s | 4,0 s | 6,5 s |

- Allein laufen die beiden parallel in 29 und 28 s. In der ganzen Suite
  sind dann alle Kerne belegt.
- Der Testprozess von `schneller_weg_gleicht_der_referenz` braucht parallel
  an der Spitze 0,21 GiB, gelesen aus `PeakWorkingSet64`.
- **Mit Optimierung,** die Abhängigkeiten mit `opt-level` 2, das Crate und
  die Tests mit 1: Der erste Bau nach dem Wechsel braucht 146 s, neu bauen
  nach dem Berühren von `metatile.rs` 9 und 10 s, wie ohne. Am längsten
  laufen dann Tests der Pyramide aus `tests/cli.rs`, bis 9 s.

Mit und ohne Testbauten mit Optimierung, je ein Lauf am Stand `7e66363`
mit dem Assert in `kameras()`: eine echte Änderung in `ChunkCache::zelle`
gebaut, zurückgesetzt, die Rückkehr gestoppt neu gebaut, dann die Suite.
Ohne heisst `--config` mit `opt-level` 0 für beide Profile, in eigenem
Zielverzeichnis. Andere Programme lasteten den Rechner dabei mit 23 bis
39 % aus; der Unterschied ist viermal grösser als das.

| | ohne | mit |
|---|---|---|
| neu bauen | 15,9 s | 14,3 s |
| Suite | 89,7 s | 22,0 s |
| zusammen | 105,6 s | 36,3 s |

## CI

Je ein Lauf aus der API von GitHub, Dauer der Schritte und Jobs, dazu aus
den Logs die Zeile `Finished` des Bauens und die Summary von nextest:

- **vorher:** `e011b50`, Kopf von #87;
- **schlank und parallel:** `1c029c6`, ohne Testprofile;
- **mit Testprofilen:** `7e66363`, zweiter Versuch desselben Laufs. Der
  erste fand nur einen Teil des Caches: `Swatinem/rust-cache` hasht die
  `Cargo.toml`, und die Abhängigkeiten bauten kalt neu.

| | vorher | schlank und parallel | mit Testprofilen |
|---|---|---|---|
| ubuntu, Tests bauen | 17 s | 18 s | 50 s |
| ubuntu, nextest | 293 s | 186 s | 24 s |
| ubuntu, Schritt Tests | 311 s | 205 s | 75 s |
| ubuntu, Job | 470 s | 357 s | 203 s |
| windows, Tests bauen | 48 s | 45 s | 143 s |
| windows, nextest | 382 s | 206 s | 51 s |
| windows, Schritt Tests | 432 s | 252 s | 195 s |
| windows, Job | 502 s | 324 s | 268 s |
| Coverage, Tests bauen | 16 s | 19 s | 59 s |
| Coverage, Schritt | 448 s | 244 s | 258 s |
| Coverage, Job | 490 s | 281 s | 318 s |

- Der Schritt `Tests (release)` ändert sich mit den Testprofilen nicht; er
  lag bei 102, 98 und 75 s.
- In der CI baut cargo ohne `incremental`. Das Crate mit `opt-level` 1 zu
  bauen kostet dort jedes Mal 31 s auf ubuntu und 98 s auf windows mehr;
  die Tests sparen 162 und 155 s.
- Coverage spart beim Testen weniger, als das Bauen kostet: 14 s mehr im
  Schritt. Die Jobs der CI blieben unverändert, siehe #90, „Nicht dabei“.
- Der erste Versuch mit kaltem Cache: Schritt Tests 228 s auf ubuntu und
  489 s auf windows, Clippy 71 und 155 s statt 4 und 15 s. Davon bauen laut
  Log im Review 198 und 432 s, nextest 30 und 56 s. Das kostet einmal je
  Änderung an einer `Cargo.toml`.

## Schluss

- Die schlanke Auswahl in `kameras()` holt lokal ein Drittel der Testzeit
  zurück. Jede Invariante prüft weiter jede Kamera aus der Vorgabe und alle
  vier Richtungen, schräg wie genordet.
- Parallel und mit Testbauten mit Optimierung braucht die Suite lokal 22
  statt 90 s, neu bauen und testen zusammen 36 statt 106 s.
- In der CI wartet man auf den längsten Job, 318 statt 357 s ohne
  Testprofile und 502 s vorher. Die Testprofile bleiben: Bauen und Testen
  werden zusammen lokal und in der CI schneller, ausser in Coverage um 14 s.
- Das Profil `mutation` baut eine Mutation rund achtmal schneller.
