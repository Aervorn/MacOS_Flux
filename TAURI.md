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
- **Menus contextuels** : faits pour les flux, dossiers, tags et articles. Reste à faire : changer l'icône et la couleur d'un dossier, changer l'adresse d'un flux, glisser-déposer un flux dans un dossier, menu sur plusieurs articles sélectionnés.
- **Archivage des favoris** : mettre un article en favori ne télécharge pas encore son texte complet ni ses images dans l'archive permanente (dépend du téléchargement des images, étape suivante).
- **Doublons : comparaison par le sens absente** : NLEmbedding n'existe pas hors macOS, seuls les liens identiques et le recouvrement des mots des titres sont utilisés (moins de regroupements que l'app Swift pour des titres formulés très différemment). À évaluer à l'usage ; piste : un petit modèle d'embeddings embarqué.
- **Filtres** : portent uniquement sur le titre (comme l'app Swift) ; pas de glisser-déposer ni d'ordre des règles (la plus ancienne l'emporte).
- **Cache d'images** : les miniatures des nouveaux articles sont téléchargées dans notre dossier (`images/`), mais le cache n'est pas encore limité en taille (réglage « limite du cache »). Les images à l'intérieur des articles se chargent en ligne à la lecture.
- **Erreurs de flux** : mémorisées en base (`last_error`) mais pas encore affichées dans la barre latérale.
- **Pas de mode hors ligne ni de préparation hors ligne** (réglage `offlineEnabled`).

## Synchronisation iCloud (en cours)
- Même format JSON v1 que l'app Swift, même dossier `iCloud Drive/Flux/devices/`.
- Limites connues : changement d'adresse d'un flux (`previousURL`) pas repris (l'ancien flux est supprimé, le nouveau créé, l'état lu de ses articles est perdu) ; un fichier iCloud pas encore téléchargé est demandé via `brctl` mais n'est lu qu'à la synchro suivante ; la synchro bloque brièvement la base pendant qu'elle tourne.
- Testée à la main (lecture, ajout/suppression de flux) : OK.
