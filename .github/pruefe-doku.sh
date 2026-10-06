#!/usr/bin/env bash
# Prüft, dass die Doku auf Bestehendes zeigt. Aus der Wurzel des Repositorys:
#
#   bash .github/pruefe-doku.sh [pfadangabe…]
#
# Weitere Pfadangaben gehen an die Suche nach Verweisen im Code, etwa
# ':!src/test/resources/*.json' für wörtliche Kopien aus einem anderen Repo.
#
# - Verweise im Code, "Siehe docs/" mit Seite und Überschrift in „“, in jeder
#   Datei ausser Markdown: die Seite und, falls genannt, die Überschrift gibt es;
# - relative Links in docs/, skills/, README.md und AGENTS.md zeigen auf eine
#   Datei, "[…](<seite>.md), „<Überschrift>“" auch auf eine Überschrift darin;
# - jede Seite in docs/ beginnt mit Frontmatter zwischen zwei ---, mit
#   title, description und code:, dieses mit mindestens einem Pfad (nur
#   docs/index.md hat keinen), und jeder Pfad existiert;
# - jede Seite in docs/ steht in docs/index.md.
#
# Jeder Fehler steht als Zeile "::error file=…::…" da; dann Exit 1.
set -u
export LC_ALL=C.UTF-8
fehler=0

melde() {
  echo "::error file=$1::$2"
  fehler=1
}

# Gibt es in $1 eine Überschrift mit genau dem Text $2?
hat_ueberschrift() {
  sed -n 's/^#\{1,6\} //p' "$1" | grep -Fxq -- "$2"
}

# Verweise im Code: jede Datei, die Git verfolgt, ausser Markdown.
while IFS= read -r zeile; do
  datei=${zeile%%:*}
  rest=${zeile#*:}
  nummer=${rest%%:*}
  if [[ $rest =~ [Ss]iehe\ (docs/[^\ ,]+\.md)(,\ „(.*)“)? ]]; then
    seite=${BASH_REMATCH[1]}
    titel=${BASH_REMATCH[3]}
    if [[ ! -f $seite ]]; then
      melde "$datei,line=$nummer" "Verweis auf $seite: die Seite gibt es nicht"
    elif [[ -n $titel ]] && ! hat_ueberschrift "$seite" "$titel"; then
      melde "$datei,line=$nummer" "Verweis auf $seite: keine Überschrift „$titel“"
    fi
  fi
done < <(git grep -n -I -E '[Ss]iehe docs/[^ ,]+\.md' -- . ':!*.md' ':!.github/pruefe-doku.sh' "$@")

# Relative Links, ohne Codeblöcke.
while IFS= read -r datei; do
  ordner=$(dirname "$datei")
  while IFS=$'\t' read -r nummer text; do
    while [[ $text =~ \]\(([^\)\ ]+)\)(,\ „([^“]*)“)? ]]; do
      ziel=${BASH_REMATCH[1]}
      titel=${BASH_REMATCH[3]}
      text=${text#*"${BASH_REMATCH[0]}"}
      case $ziel in http://* | https://* | mailto:* | \#*) continue ;; esac
      pfad="$ordner/${ziel%%#*}"
      if [[ ! -e $pfad ]]; then
        melde "$datei,line=$nummer" "Link auf $ziel: die Datei gibt es nicht"
      elif [[ -n $titel && $pfad == *.md ]] && ! hat_ueberschrift "$pfad" "$titel"; then
        melde "$datei,line=$nummer" "Link auf $ziel: keine Überschrift „$titel“"
      fi
    done
  done < <(awk '/^[[:space:]]*```/ { code = !code; next } !code { print FNR "\t" $0 }' "$datei")
done < <(git ls-files -- 'docs/*.md' 'skills/*.md' README.md AGENTS.md)

# Frontmatter und der Eintrag in docs/index.md.
while IFS= read -r seite; do
  if [[ $(head -n 1 "$seite") != --- ]] ||
    ! awk 'NR > 1 && $0 == "---" { ende = 1; exit } END { exit !ende }' "$seite"; then
    melde "$seite" "keine Frontmatter zwischen zwei ---"
  else
    kopf=$(awk 'NR > 1 && $0 == "---" { exit } NR > 1' "$seite")
    for feld in title description; do
      grep -q "^$feld: *[^ ]" <<< "$kopf" || melde "$seite" "Frontmatter ohne $feld"
    done
    pfade=$(awk '/^[a-z_]+:/ { liste = /^code:/ }
                 liste && /^  - / { sub(/^  - /, ""); print }' <<< "$kopf")
    if ! grep -q '^code:' <<< "$kopf"; then
      melde "$seite" "Frontmatter ohne code"
    elif [[ -z $pfade && $seite != docs/index.md ]]; then
      melde "$seite" "code: ohne Pfad"
    fi
    while IFS= read -r pfad; do
      [[ -z $pfad || -e $pfad ]] || melde "$seite" "code: $pfad gibt es nicht"
    done <<< "$pfade"
  fi
  if [[ $seite != docs/index.md ]] && ! grep -Fq "](${seite#docs/})" docs/index.md; then
    melde "$seite" "fehlt in docs/index.md"
  fi
done < <(git ls-files -- 'docs/*.md')

exit $fehler
