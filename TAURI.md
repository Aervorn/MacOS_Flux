# Flux — version Tauri (branche `tauri`)

Migration en cours de l'app Swift vers Tauri 2 + Rust + React + TypeScript.
L'app Swift (`Flux/`, `project.yml`) reste intacte sur cette branche.

## Prérequis
Rust (rustup, cibles `aarch64-apple-darwin` et `x86_64-apple-darwin`), Node, pnpm, Xcode.

## Commandes
```bash
pnpm install                       # dépendances du front
pnpm tauri icon Design/icon.png    # régénère src-tauri/icons ; supprimer ensuite tout sauf 32x32, 128x128, 128x128@2x, icon.icns, icon.png
pnpm check:i18n                    # fr.json et en.json ont les mêmes clés
cargo test -p flux-core            # tests du cœur Rust
pnpm tauri dev                     # lance l'app en développement
```

## Organisation
- `crates/flux-core/` : logique métier Rust, sans dépendance à Tauri.
- `src-tauri/` : coque Tauri (commandes, fenêtre, bundle).
- `src/` : interface React ; `src/styles/tokens.css` = palette (sombre mesurée, clair provisoire).
- `web-dist/` : sortie du build du front (`dist/` est réservé à `scripts/make-dmg.sh`).

## À améliorer (todo)

- **Export PDF** : l'impression passe par la fenêtre native de macOS (il faut cliquer sur le petit bouton « PDF » en bas à gauche). Piste : un vrai « Exporter en PDF » direct via `WKWebView.createPDF` (code natif Rust/objc2).
- **Vidéos intégrées** (iframes YouTube, Vimeo…) : retirées par le nettoyage du HTML ; seul le lien reste. Piste : autoriser une liste blanche de lecteurs.
- **Menu contextuel** sur les articles (clic droit : lu/non lu, favori, tags…) : reporté après les actions, **à faire avant la release finale**.
- **Archivage des favoris** : mettre un article en favori ne télécharge pas encore son texte complet ni ses images dans l'archive permanente (dépend du téléchargement des images, étape suivante).
- **Tags** : création et attribution faites ; renommer / supprimer un tag viendra avec les menus contextuels de la barre latérale.
- **Nouveaux articles : pas encore de filtres ni de détection de doublons** (règles « masquer / marquer lu » et regroupement « Aussi publié par » arrivent aux étapes suivantes) : un article qui devrait être masqué par une règle apparaît pour l'instant.
- **Cache d'images** : les miniatures des nouveaux articles sont téléchargées dans notre dossier (`images/`), mais le cache n'est pas encore limité en taille (réglage « limite du cache »). Les images à l'intérieur des articles se chargent en ligne à la lecture.
- **Erreurs de flux** : mémorisées en base (`last_error`) mais pas encore affichées dans la barre latérale.
- **Pas de mode hors ligne ni de préparation hors ligne** (réglage `offlineEnabled`).
