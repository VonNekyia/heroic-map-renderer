---
title: Grafikkarte
description: Wie --gpu die Kacheln auf der Grafikkarte zeichnen lässt, Byte für Byte wie auf der CPU, welche Adapter der Renderer nimmt und was bei einem Ausfall geschieht.
code:
  - renderer/src/render/gpu.rs
  - renderer/src/render/gpu.wgsl
  - renderer/src/render/metatile.rs
  - renderer/src/cli.rs
---

# Grafikkarte

`--gpu auto` (Standard) zeichnet die Kacheln auf der Grafikkarte, wenn eine
da ist; `--gpu off` lässt die CPU zeichnen, `--gpu on` verlangt eine Karte
und nimmt auch einen Software-Adapter. Das Bild ist in allen Fällen
dasselbe, Byte für Byte. Die Karte übernimmt nur das Zeichnen, auf der
Basis und auf den nativen Stufen; `--render` und `--pyramid` bleiben auf der
CPU, `--gpu` verlangt `--tiles`. Der Zeichner steht in
[`renderer/src/render/gpu.rs`](../../renderer/src/render/gpu.rs), der
Shader in `gpu.wgsl` daneben.

## Was die Karte zeichnet

Für die Karte stellt der Renderlauf je Kachel die Zeichenliste auf, Chunks
lesen, Kandidaten aus den Bitmasken, Sprites wählen, sortieren
(`draw_list` in `renderer/src/render/metatile.rs`), und schickt sie als
Liste von Sprites und Positionen hinüber. Ein Compute-Shader (`gpu.wgsl`)
setzt sie zusammen: die Kachel ist in Zellen von 16×16 Pixeln zerlegt, je
Zelle steht die Liste der Sprites, die sie berühren, in Zeichenreihenfolge,
und jeder Pixel-Thread geht seine Liste durch und mischt. Sechzehn Kacheln
gehen je Durchgang hinüber, mit ihnen die Sprites, die sie brauchen, jedes
einmal; die fertigen Bilder kommen zurück und werden wie bisher als WebP
geschrieben.

Eine Instanz trägt Sprite, Position, die Faktoren des Lichts, je Farbkanal
die drei Wörter des Lichts an den Ecken und die Farben des Blocks für die
Tönungskarte, 64 Bytes, siehe
[Weiche Beleuchtung](../renderer/weiche-beleuchtung.md), „Beim Zeichnen“. Hinter den Pixeln eines Sprites stehen im Puffer
seine AO-Karte und seine Tönungskarte, falls es sie hat, siehe
[Biomfarben](../renderer/biomfarben.md), „Tönung beim Zeichnen“. Die Karte
bekommt dieselben
Draws, die die Deckungsmaske der CPU behält, und zeichnet jeden ganz; was
davon verdeckt ist, übermalt ein späterer Draw mit Alpha 255, siehe
[Sprites und Deckung](../renderer/sprites-und-deckung.md), „Deckungsmaske“.

## Byte für Byte wie die CPU

Das Mischen rechnet ganzzahlig, weil Gleitkomma auf jeder Karte anders
rundet: `over` rechnet auf 1/255² erweitert und rundet einmal am Schluss, im
Shader genauso wie auf der CPU. Gegenüber der Gleitkommafassung davor weicht
das Ergebnis höchstens um 1 ab, und nur dort, wo Gleitkomma selbst daneben
lag; in den Testbildern ergab sie dieselben Pixel. Tönung, Licht und weiche
Beleuchtung rechnen ebenso ganzzahlig. Tests prüfen das auf jeder Karte, auf
der sie laufen, auch in einer Szene mit Lava in Stufen, Ackerboden neben
Lava, Draws unter Wasser, weich beleuchteten Draws und Gras und Wasser über
eine Biomgrenze. Warum so:
[0023](../entscheidungen/0023-zeichnen-auf-der-grafikkarte.md).

## Adapter und Backends

Vulkan zuerst, auf Windows wie auf Linux; DX12 nur, wenn keine echte Karte
Vulkan kann, und eine echte Karte immer vor einem Software-Adapter, eine
eigenständige vor der Onboard-Grafik. GL ist nicht dabei, siehe
[0024](../entscheidungen/0024-vulkan-zuerst-ohne-gl.md); eine Karte nur mit
GL-Treiber zeichnet auf der CPU, dasselbe Bild.

| Variable | Wirkung |
|---|---|
| `WGPU_BACKEND` | wählt die Backends |
| `WGPU_ADAPTER_NAME` | wählt einen Adapter nach einem Teil seines Namens; passt keiner, zeichnet `--gpu auto` auf der CPU und sagt warum, `--gpu on` bricht ab |
| `TERRANOVA_GPU_PFLICHT` | in der CI: ein fehlender Adapter ist ein Fehler, kein übergangener Test |

Einen Software-Adapter nimmt nur `--gpu on`, etwa WARP mit
`WGPU_ADAPTER_NAME="Basic Render"`. Ohne Karte läuft alles wie vorher auf
der CPU; die Tests, die eine Karte brauchen, überspringen sich dann und
sagen es. In CI laufen sie auf Software-Adaptern, lavapipe (Vulkan) auf
Ubuntu und WARP (DX12) auf Windows: derselbe Shader-Weg wie auf einer
echten Karte, nur langsam. Dort ist ein fehlender Adapter ein Fehler, siehe
[CI](../entwicklung/ci.md).

## Im Log und bei einem Ausfall

Im Log steht je Stufe, wie viele Kacheln die Karte gezeichnet hat:
`Threads + GPU` und `nativ bei scale 16 + GPU` für alle, sonst etwa
`+ GPU für 1200 von 1392`; `--gpu off` ist der Vergleich. Versagt die
Karte mitten im Lauf, etwa nach einem Treiber-Reset, oder antwortet sie
eine Minute lang nicht, zeichnet die CPU den Rest, mit `auto` wie mit
`on`, und das Log sagt einmal, warum. Das gilt auch für das Anlegen des
Zeichners und das Öffnen der Karte. Die Karte öffnet der Lauf vor dem
Vorlauf, damit `--gpu on` ohne Karte sofort abbricht und nicht erst nach
Minuten.

## Was es bringt und kostet

Die Karte ersetzt nur den Blit, zur Zeit von #11 rund die Hälfte der Zeit je
Kachel: Chunks dekodieren, Kandidaten sammeln, Sprite-Wahl und WebP bleiben
auf der CPU. Seit der Deckungsmaske ist die CPU ohne Karte etwa so schnell
wie mit, siehe
[2026-09-27, Die grossen Posten, zweite Runde](../messungen/2026-09-27-grosse-posten-zweite-runde.md).
Die erste Messung mit Karte steht in
[2026-09-26, Grafikkarte](../messungen/2026-09-26-grafikkarte.md): auf einem
Kern gut anderthalbmal so schnell, auf 24 Threads auf der grossen Welt 20
bis 29 % und auf einem Ausschnitt der Testwelt ein gutes Drittel. Dafür hält
jeder Thread sechzehn Zeichenlisten und braucht mehr Speicher. Eine
Onboard-Grafik ist nicht gemessen; sie teilt sich den Speicher mit der CPU,
der Gewinn dort ist also eher kleiner. Dort zeigt `--gpu off` gegen
`--gpu auto`, ob der Standard passt.
