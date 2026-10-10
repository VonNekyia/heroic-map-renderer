#!/usr/bin/env bash
# Sagt für die Notizen eines Release, welche Bäume einen vollen Lauf
# brauchen: je Look (map, cinematic, flat), ob sich sein Zeichenstand seit
# dem letzten Tag geändert hat, und für alle, ob sich eine Tabelle unter
# renderer/src/assets/ geändert hat. Vor 0101 galt ein ZEICHENSTAND für alle
# Looks; bis v0.5.0 stand keiner im Code, jene Builds zeichnen wie
# Zeichenstand 1. Den Wortlaut übernimmt das Plugin in seinen CHANGELOG.
# Siehe docs/entscheidungen/0101-zeichenstand-je-look.md.
# Aufruf: bash .github/voller-lauf.sh <Tag>
set -euo pipefail
neu=$1
alt=$(git describe --tags --abbrev=0 --match 'v*' "$neu^")
looks=(map cinematic flat)

# Je Look in der Reihenfolge von looks sein Zeichenstand an einem Stand.
staende() {
  local quelle einer look wert
  quelle=$(git show "$1:renderer/src/render/stand.rs" 2>/dev/null || true)
  einer=$(sed -n 's/^pub const ZEICHENSTAND: u32 = \([0-9]*\);$/\1/p' <<< "$quelle")
  for look in "${looks[@]}"; do
    wert=$(sed -n "s/^pub const ZEICHENSTAND_${look^^}: u32 = \([0-9]*\);\$/\1/p" <<< "$quelle")
    echo "${wert:-${einer:-1}}"
  done
}

# Die Namen mit Komma und „und“: map, cinematic und flat.
und() {
  local text=$1
  shift
  while [ $# -gt 1 ]; do
    text+=", $1"
    shift
  done
  if [ $# -eq 1 ]; then
    text+=" und $1"
  fi
  echo "$text"
}

mapfile -t vorher < <(staende "$alt")
mapfile -t jetzt < <(staende "$neu")
anders=()
gleich=()
for i in "${!looks[@]}"; do
  if [ "${vorher[$i]}" = "${jetzt[$i]}" ]; then
    gleich+=("${looks[$i]}")
  else
    anders+=("${looks[$i]}")
  fi
done
if ! git diff --quiet "$alt" "$neu" -- 'renderer/src/assets/*.txt' || [ ${#gleich[@]} -eq 0 ]; then
  echo "Jeder Baum braucht einen vollen Lauf: Zeichenstand oder Tabellen sind anders als in $alt."
elif [ ${#anders[@]} -eq 0 ]; then
  echo "Kein voller Lauf nötig: Zeichenstand und Tabellen wie in $alt, --update geht weiter." \
    "Einen Baum eines Release vor v0.4.0 rendert der Renderer einmal ganz."
else
  echo "Einen vollen Lauf brauchen nur Bäume mit look $(und "${anders[@]}"): ihr Zeichenstand ist" \
    "anders als in $alt. Bäume mit look $(und "${gleich[@]}"): kein voller Lauf, --update geht weiter."
fi
