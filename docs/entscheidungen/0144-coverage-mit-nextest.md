---
title: "0144: Coverage mit nextest, ein Prozess je Test"
description: Warum die Coverage mit cargo llvm-cov nextest läuft, nicht mit cargo llvm-cov allein, und warum der Renderer keinen Schutz gegen den Absturz des Vulkan-Loaders bekommt.
status: gilt
date: 2026-11-16
issues: [446]
code:
  - .github/workflows/ci.yml
---

# 0144: Coverage mit nextest, ein Prozess je Test

## Anlass

Der Coverage-Job stürzte in `tests/gpu.rs` oft mit SIGSEGV ab (#446). Der
Rust-Job mit denselben Tests stürzte nie ab. Die Ursache steht mit den
Zahlen in [Tests](../entwicklung/tests.md), „GPU-Tests“:

- `cargo llvm-cov` lässt die Tests eines Programms als Threads in einem
  Prozess laufen, `cargo nextest` jeden Test in einem eigenen.
- Legt ein Thread ein Vulkan-Gerät an, während ein anderer im selben
  Prozess ein Objekt benennt oder die Adapter aufzählt, stürzt der Loader
  ab.
- Der Renderer öffnet in einem Lauf nur ein Gerät. Nur der Testlauf trifft
  den Fehler.

## Entscheidung

Die Coverage läuft mit `cargo llvm-cov nextest --all-targets`, wie der
Rust-Job mit einem Prozess je Test. Am Renderer ändert sich nichts. Die
GPU-Tests bleiben in der Coverage.

## Verworfene Alternativen

- **Instanz ohne `InstanceFlags::DEBUG`:** wgpu benennt dann keine Objekte
  mehr, und der eine Wettlauf fällt weg. Es bleiben 3 Abstürze in 120
  Läufen, denn dann trifft das Aufzählen der Adapter das Anlegen eines
  Geräts. Dazu fehlten im Debug-Build die Namen in den Meldungen der
  Validierung.
- **Eine Sperre um `Gpu::mit_grenze`:** Sie deckt das Anlegen ab, aber nicht
  das Benennen und nicht das Abbauen, wenn ein Test sein Gerät freigibt,
  während ein anderer eins anlegt. Sie wäre Code im Renderer für einen Fall,
  den nur der Testlauf kennt.
- **Eine Sperre je Test in `tests/gpu.rs`:** Geräte legen auch Tests in
  `src/cli.rs`, `src/render/gpu.rs` und `tests/cli.rs` an. Jeder neue Test
  mit Karte müsste an die Sperre denken.
- **`--test-threads 1` für die Coverage:** Dann laufen alle Tests der
  Coverage nacheinander, nicht nur die GPU-Tests.
- **Den Job neu starten oder die GPU-Tests aus der Coverage nehmen:** Das
  erste lässt den Fehler stehen, das zweite nimmt dem GPU-Weg die Abdeckung.

## Folgen

- Die Coverage braucht nextest im Job.
- `cargo test` lokal kann in den GPU-Tests weiter abstürzen. Die Tests
  laufen darum nur mit nextest, siehe [Tests](../entwicklung/tests.md),
  „Laufen lassen“.
