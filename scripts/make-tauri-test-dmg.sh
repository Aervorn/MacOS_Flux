#!/bin/bash
#
# Fabrique dist/Flux-Test-<version>.dmg : la version Tauri de test, installable à côté
# de l'app Swift (autre nom « Flux Test » et autre identifiant, donc autres données).
#   1. compile l'app universelle (Apple Silicon + Intel) avec la configuration de test ;
#   2. la signe avec le certificat auto-signé et vérifie la signature ;
#   3. crée un DMG simple (app + raccourci Applications).
#
# Prérequis : rustup, pnpm, et le certificat « Aervorn Code Signing » dans le trousseau.
# Usage     : ./scripts/make-tauri-test-dmg.sh
#
# Pas de mise à jour automatique dans cette version de test, et pas de notarisation :
# au premier lancement, clic droit > Ouvrir.

set -euo pipefail
cd "$(dirname "$0")/.."

IDENTITY="Aervorn Code Signing"
TARGET="universal-apple-darwin"
APP_NAME="Flux Test"
STAGE="build/tauri-test-dmg"

step() { printf '\n\033[1;33m▸ %s\033[0m\n' "$1"; }
fail() { printf '\n\033[1;31m✗ %s\033[0m\n' "$1" >&2; exit 1; }

command -v pnpm >/dev/null || fail "pnpm est introuvable."
command -v rustup >/dev/null || fail "rustup est introuvable."
security find-identity -p codesigning | grep -q "$IDENTITY" \
    || fail "Certificat « $IDENTITY » introuvable dans le trousseau."

step "Cibles Rust (Apple Silicon + Intel)"
rustup target add aarch64-apple-darwin x86_64-apple-darwin

step "Dépendances"
pnpm install --frozen-lockfile

step "Compilation de l'app universelle (plusieurs minutes)"
pnpm tauri build --target "$TARGET" --bundles app --config src-tauri/tauri.test.conf.json

APP="src-tauri/target/$TARGET/release/bundle/macos/$APP_NAME.app"
[ -d "$APP" ] || fail "$APP_NAME.app introuvable (cherche dans src-tauri/target/$TARGET/release/bundle)."
BIN="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print CFBundleExecutable' "$APP/Contents/Info.plist")"
VERSION=$(/usr/libexec/PlistBuddy -c "Print CFBundleShortVersionString" "$APP/Contents/Info.plist")
echo "$APP_NAME $VERSION — architectures : $(lipo -archs "$BIN")"

step "Vérification de la signature"
codesign --verify --strict --deep "$APP" || fail "La signature de l'app est invalide."
REQUIREMENT=$(codesign -d -r- "$APP" 2>&1 | grep "designated" || true)
echo "$REQUIREMENT"
[[ "$REQUIREMENT" == *"certificate leaf"* ]] \
    || fail "L'app n'est pas signée avec le certificat (signature ad-hoc)."

step "Création du DMG"
rm -rf "$STAGE"
mkdir -p "$STAGE" dist
ditto "$APP" "$STAGE/$APP_NAME.app"
ln -s /Applications "$STAGE/Applications"
DMG="dist/Flux-Test-$VERSION.dmg"
rm -f "$DMG"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
codesign --sign "$IDENTITY" "$DMG"

step "Terminé"
echo "$DMG ($(du -h "$DMG" | cut -f1))"
shasum -a 256 "$DMG"
