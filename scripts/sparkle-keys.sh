#!/bin/bash
#
# À lancer une seule fois : crée la paire de clés EdDSA de Sparkle et inscrit la clé
# publique dans project.yml (SUPublicEDKey).
#
# La clé privée reste dans le trousseau de session (élément « Private key for signing
# Sparkle updates ») : c'est elle qui signe chaque DMG publié. Sauvegarde-la
# (scripts/sparkle-keys.sh --export) : sans elle, les apps déjà installées refuseront
# toutes les mises à jour suivantes.
#
# Prérequis : Xcode (pour télécharger le paquet Sparkle) et brew install xcodegen

set -euo pipefail
cd "$(dirname "$0")/.."

SPM="build/spm"
BIN="$SPM/artifacts/sparkle/Sparkle/bin"

if [ ! -x "$BIN/generate_keys" ]; then
    xcodegen generate --quiet
    xcodebuild -resolvePackageDependencies -project Flux.xcodeproj -scheme Flux \
        -clonedSourcePackagesDirPath "$SPM" -quiet
fi

if [ "${1:-}" = "--export" ]; then
    OUT="$HOME/Desktop/flux-sparkle-private-key.txt"
    "$BIN/generate_keys" -x "$OUT"
    echo "Clé privée exportée dans $OUT : range-la dans ton gestionnaire de mots de passe, puis supprime le fichier."
    exit 0
fi

# Crée la clé si elle n'existe pas encore, sinon affiche la clé publique existante
"$BIN/generate_keys" >/dev/null
PUBLIC=$("$BIN/generate_keys" -p)
[ -n "$PUBLIC" ] || { echo "Impossible de lire la clé publique." >&2; exit 1; }

sed -i '' -E "s|^( *SUPublicEDKey: ).*|\1$PUBLIC|" project.yml
echo "Clé publique : $PUBLIC"
echo "project.yml mis à jour. Lance « xcodegen generate » puis compile."
