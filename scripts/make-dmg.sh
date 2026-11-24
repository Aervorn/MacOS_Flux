#!/bin/bash
#
# Fabrique la release : dist/Flux-<version>.dmg, prête à publier.
#   1. compile l'app universelle (Apple Silicon + Intel) et signe les artefacts de mise à jour ;
#   2. la signe avec le certificat auto-signé et vérifie la signature ;
#   3. crée le DMG avec sa fenêtre d'installation (fond, icônes placées, raccourci Applications) ;
#   4. prépare les notes de version (changelog du README + SHA-256) ;
#   5. écrit dist/latest.json (mises à jour Tauri) et appcast.xml (mises à jour Sparkle,
#      pour les utilisateurs de l'ancienne app Swift).
#
# Prérequis : brew install create-dmg, rustup, pnpm, le certificat « Aervorn Code Signing »,
#             la clé de mise à jour (./scripts/setup-updater-key.sh) et les outils Sparkle
#             (sign_update : brew install --cask sparkle, ou swift/build/spm/…).
# Usage     : ./scripts/make-dmg.sh   (depuis n'importe où)
#
# Le projet reste compilable par n'importe qui sans certificat : la signature n'est ajoutée
# que par src-tauri/tauri.release.conf.json, utilisé uniquement ici.
#
# La première fois, macOS demande d'autoriser le Terminal à piloter le Finder :
# create-dmg s'en sert pour placer les icônes dans la fenêtre. Accepte.

set -euo pipefail
cd "$(dirname "$0")/.."

IDENTITY="Aervorn Code Signing"
TARGET="universal-apple-darwin"
BUNDLE="target/$TARGET/release/bundle/macos"
STAGE="build/dmg"
BACKGROUND="build/background.tiff"
UPDATER_KEY="${TAURI_SIGNING_PRIVATE_KEY_PATH:-$HOME/.tauri/flux-updater.key}"
REPO="Aervorn/MacOS_Flux"

step() { printf '\n\033[1;33m▸ %s\033[0m\n' "$1"; }
fail() { printf '\n\033[1;31m✗ %s\033[0m\n' "$1" >&2; exit 1; }

command -v create-dmg >/dev/null || fail "create-dmg est introuvable. Installe-le avec : brew install create-dmg"
command -v pnpm >/dev/null || fail "pnpm est introuvable."
command -v rustup >/dev/null || fail "rustup est introuvable."
security find-identity -p codesigning | grep -q "$IDENTITY" \
    || fail "Certificat « $IDENTITY » introuvable dans le trousseau."
[ -f "$UPDATER_KEY" ] || fail "Clé de mise à jour introuvable ($UPDATER_KEY). Lance d'abord : ./scripts/setup-updater-key.sh"
grep -q "REMPLACER_PAR_LA_CLE_PUBLIQUE" src-tauri/tauri.conf.json \
    && fail "Clé publique absente de tauri.conf.json. Lance d'abord : ./scripts/setup-updater-key.sh"

# Outil Sparkle qui signe le DMG (clé privée dans le trousseau)
SIGN_UPDATE=""
for candidate in \
    "${SPARKLE_BIN:-/nonexistent}/sign_update" \
    swift/build/spm/artifacts/sparkle/Sparkle/bin/sign_update \
    /opt/homebrew/Caskroom/sparkle/*/bin/sign_update \
    /usr/local/Caskroom/sparkle/*/bin/sign_update; do
    [ -x "$candidate" ] && { SIGN_UPDATE="$candidate"; break; }
done
[ -n "$SIGN_UPDATE" ] || fail "sign_update (Sparkle) introuvable. Installe-le avec : brew install --cask sparkle (ou définis SPARKLE_BIN)."

# Mot de passe de la clé de mise à jour
if [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD+x}" ]; then
    read -r -s -p "Mot de passe de la clé de mise à jour : " TAURI_SIGNING_PRIVATE_KEY_PASSWORD
    echo
fi
export TAURI_SIGNING_PRIVATE_KEY_PATH="$UPDATER_KEY"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD

step "Cibles Rust (Apple Silicon + Intel)"
rustup target add aarch64-apple-darwin x86_64-apple-darwin

step "Dépendances"
pnpm install --frozen-lockfile

step "Compilation de l'app universelle (plusieurs minutes)"
rm -rf "$BUNDLE"
pnpm tauri build --target "$TARGET" --config src-tauri/tauri.release.conf.json

APP="$BUNDLE/Flux.app"
[ -d "$APP" ] || fail "Flux.app introuvable dans $BUNDLE."
VERSION=$(/usr/libexec/PlistBuddy -c "Print CFBundleShortVersionString" "$APP/Contents/Info.plist")
BUILD_NUMBER=$(/usr/libexec/PlistBuddy -c "Print CFBundleVersion" "$APP/Contents/Info.plist")
BIN="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print CFBundleExecutable' "$APP/Contents/Info.plist")"
echo "Flux $VERSION (build $BUILD_NUMBER)"
echo "Architectures : $(lipo -archs "$BIN")"
# Sparkle compare ce numéro avec celui de l'app installée (6 pour la dernière version Swift)
[[ "$BUILD_NUMBER" =~ ^[0-9]+$ ]] || fail "CFBundleVersion doit être numérique pour Sparkle (bundleVersion dans tauri.conf.json)."

step "Vérification de la signature"
codesign --verify --strict --deep "$APP" || fail "La signature de l'app est invalide."
REQUIREMENT=$(codesign -d -r- "$APP" 2>&1 | grep "designated" || true)
echo "$REQUIREMENT"
[[ "$REQUIREMENT" == *"certificate leaf"* ]] \
    || fail "L'app n'est pas signée avec le certificat (signature ad-hoc)."

UPDATE_TAR="$BUNDLE/Flux.app.tar.gz"
[ -f "$UPDATE_TAR" ] && [ -f "$UPDATE_TAR.sig" ] \
    || fail "Archive de mise à jour absente (createUpdaterArtifacts) dans $BUNDLE."

step "Fond de la fenêtre (1x + Retina)"
mkdir -p build dist
tiffutil -cathidpicheck Design/dmg/background.png Design/dmg/background@2x.png -out "$BACKGROUND"

step "Création du DMG"
rm -rf "$STAGE"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/Flux.app"
DMG="dist/Flux-$VERSION.dmg"
rm -f "$DMG"

VOLICON=()
[ -f "$APP/Contents/Resources/icon.icns" ] && VOLICON=(--volicon "$APP/Contents/Resources/icon.icns")

# Fenêtre de 660 × 400 points de contenu (la hauteur inclut la barre de titre).
# Les positions des icônes correspondent à la flèche dessinée dans le fond.
create-dmg \
    --volname "Flux $VERSION" \
    ${VOLICON[@]+"${VOLICON[@]}"} \
    --background "$BACKGROUND" \
    --window-pos 200 120 \
    --window-size 660 428 \
    --icon-size 128 \
    --text-size 13 \
    --icon "Flux.app" 180 200 \
    --hide-extension "Flux.app" \
    --app-drop-link 480 200 \
    "$DMG" "$STAGE"

step "Signature du DMG"
codesign --sign "$IDENTITY" "$DMG"
codesign --verify "$DMG"

step "Notes de version"
SHA=$(shasum -a 256 "$DMG" | cut -d' ' -f1)
NOTES="dist/Flux-$VERSION-notes.md"
# Section « ### <version> » du changelog du README anglais
CHANGES=$(awk -v v="### $VERSION" '$0 == v {on=1; next} on && /^##/ {exit} on' README.md)
[ -n "$CHANGES" ] || fail "Pas de section « ### $VERSION » dans le changelog de README.md."
cat > "$NOTES" <<NOTES_EOF
## What's new

$CHANGES

## Installation

Download \`Flux-$VERSION.dmg\` below, open it and drag **Flux** onto **Applications**.

> [!WARNING]
> Flux isn't notarized by Apple: the first time, macOS blocks it. Open **System Settings › Privacy & Security** and click **Open Anyway** next to the message about Flux. Details in the [README](https://github.com/$REPO#-installation).

**SHA-256:** \`$SHA\`
NOTES_EOF
echo "$(pwd)/$NOTES"

step "Mises à jour Tauri (latest.json)"
UPDATE_ASSET="dist/Flux-$VERSION.app.tar.gz"
cp "$UPDATE_TAR" "$UPDATE_ASSET"
python3 - "$VERSION" "$REPO" "$UPDATE_TAR.sig" "$NOTES" <<'PY'
import json, sys, datetime
version, repo, sig_path, notes_path = sys.argv[1:5]
signature = open(sig_path).read().strip()
notes = open(notes_path).read().split("## Installation")[0].replace("## What's new", "").strip()
url = f"https://github.com/{repo}/releases/download/v{version}/Flux-{version}.app.tar.gz"
platform = {"signature": signature, "url": url}
latest = {
    "version": version,
    "notes": notes,
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    # Le même binaire universel sert les deux architectures
    "platforms": {"darwin-aarch64": platform, "darwin-x86_64": platform},
}
with open("dist/latest.json", "w") as f:
    json.dump(latest, f, indent=2, ensure_ascii=False)
    f.write("\n")
PY
echo "$(pwd)/dist/latest.json"

step "Mise à jour Sparkle (appcast.xml)"
# Signe les octets du DMG avec la clé privée du trousseau : sparkle:edSignature="…" length="…"
ED_ATTRS=$("$SIGN_UPDATE" "$DMG")
[[ "$ED_ATTRS" == *edSignature* ]] || fail "Signature Sparkle impossible : $ED_ATTRS"
MIN_OS=$(/usr/libexec/PlistBuddy -c "Print LSMinimumSystemVersion" "$APP/Contents/Info.plist")
PUB_DATE=$(LC_ALL=C date -u "+%a, %d %b %Y %H:%M:%S +0000")

# Changelog d'un README (section « ### <version> ») converti en HTML simple pour la fenêtre de Sparkle
changelog_html() {
    awk -v v="### $VERSION" '$0 == v {on=1; next} on && /^##/ {exit} on && /^- / {sub(/^- /, ""); print}' "$1" \
        | sed -E 's/&/\&amp;/g; s/</\&lt;/g; s/>/\&gt;/g; s/\*\*([^*]+)\*\*/<b>\1<\/b>/g; s/`([^`]+)`/<code>\1<\/code>/g; s/\[([^]]+)\]\(([^)]+)\)/<a href="\2">\1<\/a>/g; s/^(.*)$/<li>\1<\/li>/'
}
NOTES_EN="<ul>$(changelog_html README.md)</ul>"
NOTES_FR="<ul>$(changelog_html README.fr.md)</ul>"

# Une seule entrée suffit : Sparkle ne regarde que la version la plus récente
cat > appcast.xml <<APPCAST_EOF
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>Flux</title>
    <link>https://github.com/$REPO</link>
    <item>
      <title>Flux $VERSION</title>
      <pubDate>$PUB_DATE</pubDate>
      <sparkle:version>$BUILD_NUMBER</sparkle:version>
      <sparkle:shortVersionString>$VERSION</sparkle:shortVersionString>
      <sparkle:minimumSystemVersion>$MIN_OS</sparkle:minimumSystemVersion>
      <description><![CDATA[$NOTES_EN]]></description>
      <description xml:lang="fr"><![CDATA[$NOTES_FR]]></description>
      <enclosure url="https://github.com/$REPO/releases/download/v$VERSION/Flux-$VERSION.dmg" type="application/octet-stream" $ED_ATTRS />
    </item>
  </channel>
</rss>
APPCAST_EOF
xmllint --noout appcast.xml 2>/dev/null || true
echo "$(pwd)/appcast.xml"

step "Terminé"
echo "$(pwd)/$DMG"
echo "SHA-256 : $SHA"
echo
echo "Pour publier, dans cet ordre (les apps installées lisent appcast.xml sur la branche main) :"
echo "  1. gh release create v$VERSION \"$DMG\" \"$UPDATE_ASSET\" dist/latest.json --title \"Flux $VERSION\" --notes-file \"$NOTES\""
echo "  2. git add appcast.xml && git commit -m \"Appcast $VERSION\" && git push"
