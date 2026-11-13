---
title: libwebp mit SIMD unter Linux
description: Packen und Dekodieren von Kacheln unter Linux mit allen SIMD-Stufen von libwebp, nur mit SSE2 wie im Release und ohne SIMD, in der CI gemessen; SSE4.1 und AVX2 bringen gegenüber SSE2 nichts Messbares, die Bytes sind gleich.
date: 2026-11-13
commits: [2af3a617]
code:
  - renderer/vendor/libwebp-sys/build.rs
  - renderer/src/render/tiles.rs
---

# libwebp mit SIMD unter Linux

Unter Linux baut das Release libwebp nur mit SSE2. Mit SSE4.1 und AVX2
dazu, gewählt zur Laufzeit wie unter Windows, packt es Kacheln nicht
messbar schneller: schnell 0,4 % langsamer bis 1,1 % schneller, kompakt
0,1 % langsamer bis 0,5 % schneller, bei einer Streuung bis 5 %. Das
Dekodieren wird 1,3 bis 2,2 % schneller. Die Bytes sind in jeder Stufe
gleich.

## Aufbau

- **Frage:** Lohnt es, unter gcc und clang die Dateien von libwebp für
  SSE4.1 und AVX2 je mit ihrem Schalter zu bauen, statt nur SSE2 zu nutzen?
  Der volle Lauf verbringt ein Viertel bis ein Drittel seiner CPU-Zeit im
  Kodieren, siehe [Renderzeit je Funktion](2026-11-13-renderzeit-je-funktion.md).
- **Stand:** der Tag `versuch/libwebp-simd-linux`, `2af3a617`, auf master
  `bd1c0a58`. Er ändert `do_build` in
  [`build.rs`](../../renderer/vendor/libwebp-sys/build.rs): Unter gcc und
  clang bekommen die Dateien `*_sse41.c` und `*_avx2.c` ihren Schalter, alle
  anderen keinen; libwebp wählt die Fassung zur Laufzeit. Nicht gemergt.
- **Kacheln:** 220 Ausschnitte von 256 × 256 Pixeln aus den Bildern unter
  `docs/bilder` und den Goldbildern. Keine Kacheln eines echten Baums: ohne
  Transparenz, zum Teil aus verlustbehaftet gespeicherten Bildern.
- **Stufen,** je ein eigener Prozess, gezwungen über den Zeiger
  `VP8GetCPUInfo` von libwebp vor dem ersten Kodieren:
  - alle: was die CPU kann, hier SSE2, SSE4.1 und AVX2;
  - nur SSE2: wie das Release unter Linux;
  - ohne SIMD.
- **Arbeit je Lauf:** `encode_webp` in
  [`tiles.rs`](../../renderer/src/render/tiles.rs) über alle Kacheln, schnell
  zwölfmal und kompakt viermal; ab dem zweiten Durchgang dazu `decode_webp`
  über die gepackten Kacheln, vierzigmal. Ein Thread.
- **Rechner:** die CI, `ubuntu-latest`, Release-Build mit gcc. Jeder
  Durchgang lief auf einem anderen geteilten Rechner; verglichen wird nur
  innerhalb eines Durchgangs.

## Ablauf

Am 13.11.2026, drei Durchgänge, je einer je Stand des Zweigs. In jedem
laufen die drei Stufen dreimal im Wechsel. Die Zahlen sind die Ausgabe des
Tests `probe_simd` im Log des Workflows; je Stufe steht der Median der drei
Läufe. Ob AVX2 gewählt war, zeigt der dritte Durchgang: Die Zeiger
`VP8LVectorMismatch` und `VP8LConvertBGRAToRGBA` aus den Tabellen von
libwebp zeigen je Stufe auf eine andere Fassung.

## Ergebnis

Millisekunden je Kachel, Median aus drei Läufen.

| Durchgang | Arbeit | alle Stufen | nur SSE2 | ohne SIMD | alle gegen nur SSE2 |
|---|---|---|---|---|---|
| 1 | Packen schnell | 2,675 | 2,706 | 3,040 | −1,1 % |
| 1 | Packen kompakt | 10,087 | 10,139 | 10,184 | −0,5 % |
| 2 | Packen schnell | 1,872 | 1,865 | 2,077 | +0,4 % |
| 2 | Packen kompakt | 7,154 | 7,170 | 7,203 | −0,2 % |
| 2 | Dekodieren, schnell gepackt | 0,631 | 0,640 | 0,803 | −1,4 % |
| 2 | Dekodieren, kompakt gepackt | 0,527 | 0,539 | 0,553 | −2,2 % |
| 3 | Packen schnell | 2,235 | 2,251 | 2,532 | −0,7 % |
| 3 | Packen kompakt | 8,461 | 8,454 | 8,461 | +0,1 % |
| 3 | Dekodieren, schnell gepackt | 0,667 | 0,676 | 0,945 | −1,3 % |
| 3 | Dekodieren, kompakt gepackt | 0,606 | 0,615 | 0,634 | −1,5 % |

- **Streuung:** Die drei Läufe einer Stufe liegen beim Packen meist bis
  2 % auseinander, im zweiten Durchgang einmal 3,7 % (schnell, nur SSE2)
  und einmal 4,8 % (kompakt, ohne SIMD).
- **Bytes:** In allen Läufen aller Stufen gleich, schnell 11 997 024 Byte
  und kompakt 10 226 494 Byte für die 220 Kacheln, mit demselben Hash.
- **SSE2 gegen gar kein SIMD:** Packen schnell 10 bis 11 % schneller,
  Dekodieren schnell gepackter Kacheln 20 bis 29 %, Packen kompakt unter
  1 %.

## Schluss

- **Packen:** SSE4.1 und AVX2 bringen gegenüber SSE2 keinen Unterschied
  über die Streuung.
- **Dekodieren:** 1,3 bis 2,2 % schneller. Dekodiert wird nur für die
  Pyramide, rund 1 % der CPU-Zeit eines vollen Laufs.
- **Bytes:** gleich, damit auch unter Linux mit gcc belegt, was
  [Kompakt packen](2026-10-08-kompakt.md) unter Windows zeigte.
- **Nicht übernommen:** Der Umbau des Bauskripts bringt dem Render nichts
  Messbares.
- **Grenzen:** Ausschnitte aus Bildern statt echter Kacheln, ein Thread,
  geteilte Rechner der CI.
