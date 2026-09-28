---
title: "0034: Eigene Lizenz: nutzen ja, verkaufen und übernehmen nein"
description: Warum der Renderer unter einer eigenen Lizenz steht statt unter MIT oder einer fertigen Lizenz, und welcher fremde Code dazu passt.
status: gilt
date: 2026-09-28
issues: []
code:
  - LICENSE
  - renderer/Cargo.toml
  - renderer/deny.toml
---

# 0034: Eigene Lizenz: nutzen ja, verkaufen und übernehmen nein

## Anlass

`renderer/Cargo.toml` nannte `license = "MIT"`, eine Lizenzdatei gab es
nicht. Der Maintainer will:

- Jeder darf den Renderer nutzen, auch für einen Minecraft-Server, der Geld
  einnimmt.
- Niemand verkauft ihn oder macht mit ihm Geld.
- Niemand übernimmt seinen Code in andere Projekte.

## Entscheidung

Der Renderer steht unter einer eigenen Lizenz in
[`LICENSE`](../../LICENSE).

- **Erlaubt:** ausführen, Karten zeigen, für den eigenen Einsatz ändern,
  unverändert weitergeben und für einen Pull Request forken.
- **Ausdrücklich erlaubt:** der Einsatz für einen Minecraft-Server mit
  Einnahmen.
- **Verboten ohne Erlaubnis:**
  - die Software verkaufen;
  - Rendern als bezahlten Dienst für andere anbieten;
  - gerenderte Karten verkaufen;
  - Code in andere Projekte übernehmen;
  - geänderte Fassungen veröffentlichen.

`renderer/Cargo.toml` verweist mit `license-file` auf die Datei und hat
`publish = false`. cargo-deny prüft die eigene Crate deshalb nicht, nur die
Abhängigkeiten (`[licenses.private]` in `renderer/deny.toml`).

## Verworfene Alternativen

- **[MIT](https://opensource.org/license/mit):** erlaubt Verkaufen und
  Übernehmen.
- **[PolyForm Noncommercial 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0/):**
  - verbietet jeden kommerziellen Zweck, also auch den Server mit Einnahmen;
  - erlaubt, geänderte Fassungen weiterzugeben.
- **[PolyForm Shield 1.0.0](https://polyformproject.org/licenses/shield/1.0.0/):**
  verbietet nur Produkte, die mit dem Renderer konkurrieren. Verkaufen als
  Teil eines anderen Produkts bliebe erlaubt.
- **[Commons Clause](https://commonsclause.com/) auf MIT:** verbietet das
  Verkaufen. Übernehmen und geänderte Fassungen blieben erlaubt.
- **[CC BY-NC-ND 4.0](https://creativecommons.org/licenses/by-nc-nd/4.0/):**
  - Creative Commons rät selbst davon ab, ihre Lizenzen für Software zu
    nutzen;
  - NC verbietet den Server mit Einnahmen.

## Folgen

- **Nicht Open Source:** Der Renderer ist nicht Open Source nach der
  [Open Source Definition](https://opensource.org/osd). Der Quelltext ist
  offen, die Rechte sind eingeschränkt.
- **Fremder Code:** passt nur unter freizügigen Lizenzen, samt ihren
  Hinweisen: MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0. Copyleft wie GPL
  passt nicht, weil es verlangt, das Ganze unter dieselbe Lizenz zu
  stellen.
- **Beiträge Dritter** stehen unter dieser Lizenz. Der Urheber darf sie
  auch anders lizenzieren.
