---
name: doku-pflegen
description: Hält docs/ und die Code-Kommentare mit einer Änderung im Einklang. Nutzen bei jeder PR, die Verhalten, Schalter, Ausgabe, Dateiformate, Leistung oder Grösse ändert, und im Review einer solchen PR.
---

# Doku pflegen

## Ablauf

1. **Betroffene Seiten finden.** Für jede geänderte Datei die Seiten, die
   sie in `code:` nennen: `git grep -l "renderer/src/render/metatile.rs" docs/`.
   Dazu die Seiten, auf die Kommentare im geänderten Code verweisen:
   `git grep -h "Siehe docs/" -- <datei>`.
2. **Wissen nachziehen.** Auf jeder betroffenen Seite prüfen: Stimmen der
   erste Absatz, Zahlen, Schalter, Beispielausgaben und die Liste `code:`
   noch?
3. **Neues Thema?** Neue Seite nach der Vorlage unten, dazu eine Zeile in
   `docs/index.md`.
4. **Entscheidung getroffen?** Skill `entscheidung-festhalten`.
5. **Gemessen?** Skill `messung-protokollieren`.
6. **Spielverhalten nachgebaut oder behauptet?** Skill
   `spielverhalten-belegen`.
7. **Näherungen** und Abweichungen vom Spiel stehen auf der Seite des Themas
   unter „Was bleibt eine Näherung“, jeweils mit Grund.
8. **Kommentare.** Erzählt ein Kommentar ein Warum, den Text auf die Seite
   ziehen und durch einen Verweis ersetzen. Ist eine Seite verschoben oder
   eine Überschrift umbenannt: `git grep "docs/<alter-pfad>"` und alle
   Treffer nachziehen.
9. **README** nur ändern, wenn sich Schnellstart, Stand oder das Bild des
   Projekts ändern.
10. **Verweise prüfen:** Jeder Link, jeder Verweis „Siehe docs/…“ und jeder
    Pfad unter `code:` zeigt auf etwas, das es gibt. Das prüft
    `bash .github/pruefe-doku.sh`, lokal wie in der CI; eine neue Seite erst
    nach `git add`.
11. **Prüfliste** unten abhaken.

## Vorlage für eine Seite

Pfade in `code:` relativ zum Wurzelverzeichnis.

```markdown
---
title: Wasser und Licht
description: Wie der Renderer Wasser, Himmelslicht und Blocklicht wie das Spiel rechnet.
code:
  - renderer/src/render/metatile.rs
  - renderer/src/assets/fluid.rs
---

# Wasser und Licht

Ein Absatz: was gilt, wo es im Code steht, was es kostet.

## …

## Was bleibt eine Näherung
```

## Prüfliste

Für die eigene PR und im Review:

- [ ] Für jede geänderte Datei die Seiten mit ihrem Pfad gelesen und
      nachgezogen.
- [ ] Neue Seiten stehen in `docs/index.md`.
- [ ] Neue Entscheidungen und Messungen haben ihre Datei.
- [ ] Kommentare verweisen, statt Entscheidungen zu erzählen. Jeder Verweis
      zeigt auf eine bestehende Seite und Überschrift.
- [ ] Zeichnet die PR anders, schreibt sie andere Höhen oder einen anderen
      Abdruck je Chunk, auch ohne neues Goldbild? Dann ist `ZEICHENSTAND` in
      `renderer/src/render/stand.rs` gehoben. Kodiert sie anders, ist
      `KODIERSTAND` in `renderer/src/cli/pixel.rs` gehoben. Siehe
      [0098](../../docs/entscheidungen/0098-der-zeichenstand-statt-des-builds.md).
- [ ] Keine Interna, siehe `AGENTS.md`, Regel 20.
- [ ] Links, Verweise und Pfade unter `code:` zeigen auf Bestehendes.
