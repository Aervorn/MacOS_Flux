#!/bin/bash
#
# Fabrique dist/Flux-<version>.dmg, prêt à publier :
#   1. génère le projet Xcode (XcodeGen) ;
#   2. archive Release (app universelle), signée avec le certificat auto-signé ;
#   3. vérifie que la signature n'est pas ad-hoc ;
#   4. crée le DMG avec sa fenêtre d'installation (fond, icônes placées, raccourci Applications) ;
#   5. signe le DMG et prépare les notes de version (changelog du README + SHA-256) ;
#   6. signe le DMG pour Sparkle (EdDSA) et réécrit appcast.xml.
#
# Prérequis : brew install create-dmg xcodegen
# Usage     : ./scripts/make-dmg.sh   (depuis n'importe où)
#
# Le projet reste en « Sign to Run Locally » pour que n'importe qui puisse le compiler :
# le certificat n'est utilisé que par ce script.
#
# La première fois, macOS demande d'autoriser le Terminal à piloter le Finder :
# create-dmg s'en sert pour placer les icônes dans la fenêtre. Accepte.

set -euo pipefail
cd "$(dirname "$0")/.."

IDENTITY="Aervorn Code Signing"
BUILD="build/release"
ARCHIVE="$BUILD/Flux.xcarchive"
STAGE="$BUILD/dmg"
BACKGROUND="$BUILD/background.tiff"
# Paquets Swift (Sparkle) hors de $BUILD, pour ne pas les retélécharger à chaque fois
SPM="build/spm"
SPARKLE_BIN="$SPM/artifacts/sparkle/Sparkle/bin"
REPO="Aervorn/MacOS_Flux"

step() { printf '\n\033[1;33m▸ %s\033[0m\n' "$1"; }
fail() { printf '\n\033[1;31m✗ %s\033[0m\n' "$1" >&2; exit 1; }

command -v create-dmg >/dev/null || fail "create-dmg est introuvable. Installe-le avec : brew install create-dmg"
command -v xcodegen >/dev/null || fail "xcodegen est introuvable. Installe-le avec : brew install xcodegen"
security find-identity -p codesigning | grep -q "$IDENTITY" \
    || fail "Certificat « $IDENTITY » introuvable dans le trousseau."
grep -q "SUPublicEDKey: SPARKLE_PUBLIC_KEY_MISSING" project.yml \
    && fail "Clé Sparkle absente de project.yml. Lance d'abord : ./scripts/sparkle-keys.sh"

step "Projet Xcode"
xcodegen generate --quiet

step "Archive Release"
rm -rf "$BUILD"
mkdir -p "$BUILD" dist
# « generic » : app universelle (Apple Silicon + Intel), comme « Any Mac » dans Xcode.
xcodebuild -project Flux.xcodeproj -scheme Flux -configuration Release \
    -destination "generic/platform=macOS" \
    -archivePath "$ARCHIVE" -clonedSourcePackagesDirPath "$SPM" archive -quiet \
    CODE_SIGN_IDENTITY="$IDENTITY" CODE_SIGN_STYLE=Manual

APP="$ARCHIVE/Products/Applications/Flux.app"
[ -d "$APP" ] || fail "Flux.app introuvable dans l'archive."
VERSION=$(/usr/libexec/PlistBuddy -c "Print CFBundleShortVersionString" "$APP/Contents/Info.plist")
BUILD_NUMBER=$(/usr/libexec/PlistBuddy -c "Print CFBundleVersion" "$APP/Contents/Info.plist")
echo "Flux $VERSION (build $BUILD_NUMBER)"
echo "Architectures : $(lipo -archs "$APP/Contents/MacOS/Flux")"

step "Vérification de la signature"
codesign --verify --strict --deep "$APP" || fail "La signature de l'app est invalide."
REQUIREMENT=$(codesign -d -r- "$APP" 2>&1 | grep "designated" || true)
echo "$REQUIREMENT"
[[ "$REQUIREMENT" == *"certificate leaf"* ]] \
    || fail "L'app n'est pas signée avec le certificat (signature ad-hoc)."

step "Fond de la fenêtre (1x + Retina)"
tiffutil -cathidpicheck Design/dmg/background.png Design/dmg/background@2x.png -out "$BACKGROUND"

step "Création du DMG"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/Flux.app"
DMG="dist/Flux-$VERSION.dmg"
rm -f "$DMG"

VOLICON=()
[ -f "$APP/Contents/Resources/AppIcon.icns" ] && VOLICON=(--volicon "$APP/Contents/Resources/AppIcon.icns")

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
> Flux isn't notarized by Apple: the first time, macOS blocks it. Open **System Settings › Privacy & Security** and click **Open Anyway** next to the message about Flux. Details in the [README](https://github.com/Aervorn/MacOS_Flux#-installation).

**SHA-256:** \`$SHA\`
NOTES_EOF
echo "$(pwd)/$NOTES"

step "Mise à jour Sparkle (appcast.xml)"
[ -x "$SPARKLE_BIN/sign_update" ] || fail "sign_update introuvable dans $SPARKLE_BIN."
# Signe les octets du DMG avec la clé privée du trousseau : sparkle:edSignature="…" length="…"
ED_ATTRS=$("$SPARKLE_BIN/sign_update" "$DMG")
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
echo "  1. gh release create v$VERSION \"$DMG\" --title \"Flux $VERSION\" --notes-file \"$NOTES\""
echo "  2. git add appcast.xml && git commit -m \"Appcast $VERSION\" && git push"
