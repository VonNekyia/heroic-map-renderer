---
title: "0020: mimalloc als Allokator"
description: Warum der Rust-Teil über mimalloc allokiert statt über den Heap des Systems.
status: gilt
date: 2026-09-26
issues: [10]
code:
  - renderer/src/main.rs
  - renderer/Cargo.toml
---

# 0020: mimalloc als Allokator

## Anlass

Parallel dauerte ein Chunk-Dekodieren sechsmal so lang wie allein: Der
Windows-Heap serialisiert die vielen kleinen Allokationen des NBT-Lesers
über 24 Threads.

## Entscheidung

`mimalloc` ist der globale Allokator des Binärs (`main.rs`); jeder Thread
hat seinen eigenen Heap.

## Verworfene Alternativen

- **Der Heap des Systems.** Mit ihm schaffte der Stand von damals auf 24
  Threads 245 statt 532 Kacheln/s, siehe
  [2026-09-23, Phasen je Kachel](../messungen/2026-09-23-phasen-je-kachel.md).

## Folgen

- Eine neue direkte Abhängigkeit, mimalloc (MIT); mit ihr kommen
  libmimalloc-sys (MIT) und zum Bauen cc, find-msvc-tools und shlex.
- libwebp allokiert trotzdem über `malloc` der C-Laufzeit; dafür unter
  Windows der Segment-Heap, siehe [0029](0029-segment-heap-fuer-libwebp.md).
