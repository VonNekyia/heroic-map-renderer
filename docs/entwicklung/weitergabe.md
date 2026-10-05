---
title: Weitergabe des Binärs
description: Wie das Release-Binär für eine Weitergabe gebaut ist - Release-Profil mit opt-level "s" für den GPU-Stapel und clap, statische CRT unter Windows, Grösse roh und gepackt, Linux und glibc, warum kein LTO und kein panic = "abort", und was die CI daran prüft.
code:
  - renderer/Cargo.toml
  - .cargo/config.toml
  - .github/workflows/ci.yml
---

# Weitergabe des Binärs

Das Binär geht an Leute, die keinen Rust-Compiler haben: in Releases (#150),
im Plugin auf Hangar (#153) und in der Desktop-EXE (#152). Hangar nimmt
höchstens 10 000 000 Byte je Datei an, und das Jar trägt die Binärs aller
Plattformen. Darum zählt die Grösse gepackt, mit Deflate 6 wie im Jar. Unter
Windows hat das Binär 9,15 MB, gepackt 3,57 MB, und braucht keine
VC++-Laufzeit. Was dahinter steht, misst
[Grösse des Binärs und statische CRT](../messungen/2026-10-05-binaergroesse.md).
Was jeder Weitergabe beiliegt, steht in [Drittlizenzen](drittlizenzen.md).

## Release-Profil

In [`renderer/Cargo.toml`](../../renderer/Cargo.toml):

- **`opt-level = 3` und `codegen-units = 1`** für alles, was je Pixel
  arbeitet: der eigene Code, libwebp, das Entpacken der Chunks.
- **`opt-level = "s"` für den GPU-Stapel und clap:** `naga`, `wgpu-core`,
  `wgpu-hal`, `wgpu`, `ash`, `gpu-allocator` und `clap_builder`. Sie machen
  gut die Hälfte des Binärs aus und laufen vor allem beim Start und je
  Durchgang, nicht je Pixel. Das spart gepackt 0,38 MB und kostet an der
  Testwelt mit und ohne Grafikkarte nichts Messbares.
- **Kein LTO:** Mit `codegen-units = 1` spart es gepackt nichts.
- **Kein `panic = "abort"`:** `ohne_panik` in
  [`renderer/src/cli.rs`](../../renderer/src/cli.rs) fängt Paniken, etwa
  aus wgpu, mit `catch_unwind` und macht daraus Fehler. Mit Abbruch ginge
  das nicht.
- **`opt-level = "z"`** für den GPU-Stapel spart weitere 0,19 MB gepackt,
  ist aber nicht gemessen. Es kommt nur, wenn ein Binär sonst nicht passt.

## Windows: statische CRT

[`.cargo/config.toml`](../../.cargo/config.toml) an der Wurzel des Repos
setzt für `x86_64-pc-windows-msvc` `-C target-feature=+crt-static`.

- **Wirkung:** Das Binär importiert weder `VCRUNTIME140.dll` noch
  `VCRUNTIME140_1.dll` noch die `api-ms-win-crt-*`-DLLs der UCRT. Das gilt
  auch für den C-Code von libwebp und mimalloc: Der `cc`-Crate baut ihn
  dann mit der statischen CRT. Übrig bleiben DLLs, die jedes Windows
  mitbringt, etwa `kernel32`, `advapi32` und `dxgi`.
- **Kosten:** 0,12 MB gepackt, keine messbare Laufzeit.
- **An der Wurzel,** nicht unter `renderer/`: cargo sucht die Datei ab dem
  Verzeichnis, in dem es läuft, nach oben. So gilt sie für
  `cargo build` in `renderer/` wie für `--manifest-path renderer/Cargo.toml`
  aus der Wurzel.

## Linux

Gebaut gegen glibc. Welche Version das Binär mindestens braucht, hängt an
der glibc des Rechners, auf dem es gebaut wird. Die CI meldet sie bei jedem
Lauf, siehe „In der CI“. Ob Releases auf einem älteren System oder mit musl
bauen, entscheidet #150.

## In der CI

Im Job „Rust“, siehe [CI](ci.md):

- **Ubuntu,** nach den Tests in Release: Grösse des Binärs roh und gepackt,
  die höchste glibc-Version unter seinen Symbolen und die Grösse nach
  `strip`. Der Schritt meldet nur, er fällt nicht.
- **Windows:** ein Release-Build, seine Grösse roh und gepackt. Der Schritt
  fällt, wenn das Binär `vcruntime140` oder `api-ms-win-crt-` nennt, also
  doch an der VC++-Laufzeit hängt.
