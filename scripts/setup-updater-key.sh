#!/bin/bash
#
# À lancer une seule fois : crée la paire de clés qui signe les mises à jour Tauri et
# inscrit la clé publique dans src-tauri/tauri.conf.json.
#
# La clé privée reste dans ~/.tauri/flux-updater.key (jamais dans le dépôt). Sauvegarde-la
# ainsi que son mot de passe : sans elle, les apps déjà installées refuseront toutes les
# mises à jour suivantes.
#
# Prérequis : pnpm install
# Usage     : ./scripts/setup-updater-key.sh

set -euo pipefail
cd "$(dirname "$0")/.."

KEY="$HOME/.tauri/flux-updater.key"
CONF="src-tauri/tauri.conf.json"

mkdir -p "$HOME/.tauri"
if [ -f "$KEY" ]; then
    echo "La clé existe déjà : $KEY"
else
    echo "Choisis un mot de passe (il te sera redemandé à chaque release)."
    pnpm tauri signer generate -w "$KEY"
fi
[ -f "$KEY.pub" ] || { echo "✗ $KEY.pub introuvable." >&2; exit 1; }

PUB=$(tr -d '\n' < "$KEY.pub")
python3 - "$CONF" "$PUB" <<'PY'
import json, sys
path, pub = sys.argv[1], sys.argv[2]
conf = json.load(open(path))
conf["plugins"]["updater"]["pubkey"] = pub
with open(path, "w") as f:
    json.dump(conf, f, indent=2)
    f.write("\n")
PY
echo "✓ Clé publique inscrite dans $CONF"
echo "Pense à sauvegarder $KEY (et le mot de passe) dans ton gestionnaire de mots de passe."
