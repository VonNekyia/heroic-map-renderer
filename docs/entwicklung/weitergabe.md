---
title: Weitergabe des Binärs
description: Wie das Release-Binär für eine Weitergabe gebaut ist - Release-Profil mit opt-level "s" für den GPU-Stapel und clap und ohne Symbole, statische CRT unter Windows, Grösse roh und gepackt, Linux und glibc, die Grenze für das Jar, warum kein LTO und kein panic = "abort", und was die CI daran prüft.
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
VC++-Laufzeit; unter Linux gepackt 3,49 MB. Was dahinter steht, misst
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
- **`strip = "symbols"`:** Unter Linux liegen die Symbole im Binär. Ohne
  sie hat es 8,39 statt 11,01 MB, gepackt 3,49 statt 3,90 MB, und passt so
  unter die Grenze, siehe „Grenze“. Unter Windows ändert es nichts, die
  Symbole liegen dort in der PDB. Ein Backtrace unter Linux nennt dann
  keine Namen; die Meldung einer Panik bleibt.
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
- **Nicht mit `RUSTFLAGS`:** Ist die Umgebungsvariable gesetzt, nimmt cargo
  nur sie und lässt `target.<triple>.rustflags` aus `.cargo/config.toml`
  ganz weg, statt beides zu verbinden. Wer etwa mit
  `RUSTFLAGS=-Ctarget-cpu=native` baut, bekommt still wieder die
  VC++-Laufzeit. Dann `-C target-feature=+crt-static` mit in `RUSTFLAGS`
  schreiben. Die Prüfung in der CI deckt nur ihren eigenen Build; ein
  Release prüft sein Binär selbst (#150).

## Linux

Gebaut gegen glibc. Welche Version das Binär mindestens braucht, hängt an
der glibc des Rechners, auf dem es gebaut wird, und an den Funktionen, die
es nutzt. Auf `ubuntu-latest` braucht es höchstens `GLIBC_2.34`, läuft also
ab glibc 2.34. Die CI meldet das bei jedem Lauf, siehe „In der CI“.

Releases bauen deshalb im Container `manylinux_2_28` gegen glibc 2.28 und
laufen ab glibc 2.28. Der Release-Workflow fällt, wenn das Binär mehr
verlangt. Ohne musl, siehe
[0082](../entscheidungen/0082-versionen-und-releases.md).

## Grenze

- **Das Budget:** Hangar nimmt höchstens 10 000 000 Byte je Datei an (#153).
  Das Jar trägt beide Binärs, dazu Plugin, Seite und Hinweise, zusammen
  rund 0,7 MB. Für die Binärs bleiben so gepackt
  10 000 000 − 700 000 = 9 300 000 Byte zusammen.
- **Die Grenzen,** entschieden am 06.10. im Review zu #181, die Warnung in
  der CI angehoben im Review zu #192:

  | Wo | Grenze, gepackt | Warum |
  |---|---|---|
  | Release-Workflow, beide Binärs zusammen | 9 300 000 Byte | das Jar |
  | Release-Workflow, je Binär | 4 750 000 Byte | Linux ist kleiner als Windows; keins soll allein das Budget tragen |
  | CI, Job „Rust“ unter Ubuntu | 4 750 000 Byte | eine frühe Warnung an jeder PR, gegen glibc 2.39 gebaut; dieselbe Grenze je Binär wie im Release |

  Die eigentliche Grenze ist die Summe. Die Warnung lag zuerst bei 4 550 000
  Byte, aus der Zeit, als jedes Binär für sich gemessen wurde, vor dem
  Budget als Summe. Mit Client-Jar und Assistent stand der Job „Rust“ auf
  master bei 4 547 199 Byte, und jede weitere Zeile hätte ihn fallen lassen,
  obwohl die Summe 0,22 MB Luft hatte.

  Die Summe prüft der Job „Budget beider Binärs“ in
  [`.github/workflows/release.yml`](../../.github/workflows/release.yml),
  siehe [CI](ci.md), „Release“.
- **Der Server aus #151:** HTTP mit hyper und tokio schätzt die Recherche
  auf 0,25 MB gepackt, TLS auf 0,57 MB dazu, siehe
  [0084](../entscheidungen/0084-server-im-renderer.md). Mit ihm und dem
  Download der Assets aus #147, je Binär rund 0,08 MB, passt die Summe.
- **Stand** mit `--serve` und HTTPS, gepackt, die Releases am Kopf
  `2cb14c2` von #183, der Job „Rust“ am Kopf `b4153f0`:

  | Binär | gepackt | Luft |
  |---|---|---|
  | Linux, Release gegen glibc 2.28 | 4 318 910 Byte | 431 090 unter 4 750 000 |
  | Linux, Job „Rust“ gegen glibc 2.39 | 4 441 878 Byte | 308 122 unter 4 750 000 |
  | Windows, Release | 4 522 955 Byte | 227 045 unter 4 750 000 |
  | beide Releases zusammen | 8 841 865 Byte | 458 135 unter 9 300 000 |

  Ohne TLS, am Kopf `f17d78e` von #181, waren es unter Linux 3 724 777 Byte,
  im Job „Rust“ 3 850 626 und unter Windows 3 974 591. TLS mit rustls und
  ring brachte so unter Linux 0,59 MB, unter Windows 0,55 MB. Vor dem
  Server waren es unter Linux 3,49 MB und unter Windows 3,57 MB. Dazu kamen
  `--estimate`, die eigenen Laubfarben, das Manifest und der Server, unter
  Linux 0,36 MB, unter Windows 0,40 MB; getrennt gemessen ist der Server
  nicht.

## In der CI

Im Job „Rust“, siehe [CI](ci.md):

- **Ubuntu,** nach den Tests in Release: Grösse des Binärs roh und gepackt
  und die höchste glibc-Version, die es braucht. Der Schritt fällt, wenn es
  gepackt über der Grenze liegt, siehe „Grenze“.
- **Windows,** nach den Tests: das Debug-Binär, das `nextest` ohnehin baut.
  Die statische CRT gilt für alle Profile. Der Schritt fällt, wenn das
  Binär `vcruntime140`, `ucrtbase` oder `api-ms-win-crt-` nennt, also doch
  an der VC++-Laufzeit hängt; die Debug-Laufzeit heisst `vcruntime140d` und
  `ucrtbased`. Ein eigener Release-Build dafür kostete einen zweiten Build
  je Lauf; die Grösse unter Windows misst der Release-Workflow (#150).
