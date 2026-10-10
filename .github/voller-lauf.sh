#!/usr/bin/env bash
# Sagt für die Notizen eines Release, ob jeder Baum einen vollen Lauf
# braucht: ob sich seit dem letzten Tag ZEICHENSTAND oder eine Tabelle unter
# renderer/src/assets/ geändert hat. Bis v0.5.0 stand kein Zeichenstand im
# Code; jene Builds zeichnen wie Zeichenstand 1.
# Siehe docs/entscheidungen/0098-der-zeichenstand-statt-des-builds.md.
# Aufruf: bash .github/voller-lauf.sh <Tag>
set -euo pipefail
neu=$1
alt=$(git describe --tags --abbrev=0 --match 'v*' "$neu^")
stand() {
  git show "$1:renderer/src/render/stand.rs" 2>/dev/null \
    | sed -n 's/^pub const ZEICHENSTAND: u32 = \([0-9]*\);$/\1/p' || true
}
vorher=$(stand "$alt")
vorher=${vorher:-1}
jetzt=$(stand "$neu")
if [ "$vorher" = "$jetzt" ] && git diff --quiet "$alt" "$neu" -- 'renderer/src/assets/*.txt'; then
  echo "Kein voller Lauf nötig: Zeichenstand und Tabellen wie in $alt, --update geht weiter." \
    "Einen Baum eines Release vor v0.4.0 rendert der Renderer einmal ganz."
else
  echo "Jeder Baum braucht einen vollen Lauf: Zeichenstand oder Tabellen sind anders als in $alt."
fi
