# Flux — version Tauri (branche `tauri`)

App Tauri 2 + Rust + React + TypeScript (depuis la 0.8.0, c'est l'app principale).
L'ancienne app Swift est dans `swift/` et au tag `swift-final`.

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

- **Export PDF** : fait (`src-tauri/src/pdf.rs`) — fenêtre d'enregistrement, puis `NSPrintOperation` de la WKWebView en travail « enregistrer » (objc2, appels bruts), avec la mise en page d'impression. L'opération se termine en arrière-plan : on attend que le fichier cesse de grossir (40 s max).
- **Vidéos intégrées** : faites (`src/reader/video.ts`) pour YouTube, Vimeo, Dailymotion — emplacement « Lire la vidéo », lecteur créé au clic (adresse reconstruite depuis l'identifiant, sandbox, youtube-nocookie). Les autres lecteurs (Twitch, PeerTube, lecteurs maison de certains journaux) restent des liens.
- **Menus contextuels** : faits pour les flux, dossiers, tags, articles et sélections multiples. Faits aussi : icône (symbole ou emoji) et couleur d'un dossier (`FolderStyleDialog`), glisser-déposer d'un flux vers un dossier — suivi à la souris (le glisser-déposer HTML de la WebView refuse le dépôt) avec ouverture d'un dossier fermé après 0,6 s.
- **Archivage des favoris** : fait (`archive.rs`) — images (miniature + 50 du texte) dans `archive/`, texte complet récupéré par l'interface (`offline.ts`). Un favori garde ce qui a pu être téléchargé à l'ajout ; pas de nouvel essai pour une page qui a échoué.
- **Doublons par le sens** : fait (`semantic.rs`, NLEmbedding via objc2, macOS seulement). Les groupes existants ne sont recalculés qu'au changement de sensibilité.
- **Filtres** : portent uniquement sur le titre (comme l'app Swift) ; pas de glisser-déposer ni d'ordre des règles (la plus ancienne l'emporte).
- **Dossiers intelligents** : `smart.rs` (tables `smart_folders`, `smart_folder_keywords`, `smart_folder_articles`, migration v2). Un dossier a une liste de mots-clés (un seul suffit) cherchés dans le titre, avec l'option « mot entier ». L'appartenance est recalculée en entier par `smart::rebuild`, appelée à la fin de `filters::apply` (nouveaux articles, règles d'exclusion, synchro) et à chaque changement d'un dossier. Un article rangé quitte la vue de son flux et du dossier de son flux (liste, compteurs, « tout marquer comme lu ») mais reste dans « Tous » et « Non lus » ; il est dans chaque dossier qui correspond ; un article masqué par une règle d'exclusion n'y est pas. Limites : titre seulement, pas de portée par flux, synchronisés entre Mac depuis le format v2 (nom, mots-clés, option, icône et couleur ; l'appartenance est recalculée sur chaque Mac) et inclus dans la sauvegarde (base SQLite).
- **Recherche** : options « titre » et/ou « corps » (réglages `searchInTitle` / `searchInBody`). Le corps est le texte indexé (`search_text`) sans le titre qui le commence ; les articles dont le texte complet n'a pas encore été récupéré ne sont cherchés que dans le résumé et le contenu du flux.
- **Résumé en liste** : réglage `summaryLines` (0 à 4, 2 par défaut) à la place de `showSummary` (migré au chargement : non = 0, oui = 2). Le résumé stocké est limité à 280 caractères : à 4 lignes dans une liste large, il peut tenir en moins de 4 lignes.
- **Cache d'images** : limite 250 Mo–5 Go, suppression des moins récemment utilisées (mtime « touché » à chaque affichage), bouton Vider (`offline.rs`). L'archive des favoris est un autre dossier, jamais touchée.
- **Erreurs de flux** : pastille dans la barre latérale (infobulle : message et date de la dernière actualisation réussie) et « Voir l’erreur… » dans le menu du flux. Seule la dernière erreur est conservée.
- **Mode hors ligne** : fait. Texte complet des non lus et favoris (400 max) extrait par l'interface (Readability tourne dans la WebView), puis images ; détection de la connexion par une requête vers captive.apple.com (navigator.onLine est peu fiable dans la WebView). Une page dont l'extraction échoue n'est pas retentée avant le prochain lancement.

## Synchronisation iCloud (en cours)
- Format JSON **v2** (depuis la 0.8.5, pour les dossiers intelligents), dossier `iCloud Drive/Flux/devices/`. Il casse la compatibilité avec l'app Swift et les versions 0.8.0 / 0.8.1 (format v1) : un fichier d'une autre version est ignoré et signalé dans Réglages › Synchronisation (`SyncReport::incompatible`). Les deux Mac doivent donc passer en 0.8.5 ; entre-temps ils ne se voient plus. Les anciens fichiers v1 restent dans le dossier, sans effet.
- Le changement d'adresse d'un flux (`previousURL`) est repris : l'autre Mac fait la même migration (`manage::migrate_feed`) avant le traitement des pierres tombales, donc articles, états et tags sont conservés. Si les deux adresses existent déjà sur l'autre Mac, l'ancienne est simplement supprimée.
- Limites connues : un fichier iCloud pas encore téléchargé est demandé via `brctl` mais n'est lu qu'à la synchro suivante ; la synchro bloque brièvement la base pendant qu'elle tourne.
- Testée à la main (lecture, ajout/suppression de flux) : OK.

## Sauvegarde iCloud (en cours)
- Une archive `.zip` par jour dans `iCloud Drive/Flux/Sauvegardes Tauri/<Mac>/` (dossier distinct de celui de l'app Swift : formats incompatibles). Contient la base SQLite (réglages compris), `Infos.json` et, en option, les images des favoris.
- Restauration appliquée au lancement suivant (base actuelle gardée dans `before-restore/`, identifiant de synchro conservé, mémoire de synchro effacée). En développement (`tauri dev`), la relance est manuelle.
- Limites : une restauration ne peut pas lire les sauvegardes de l'app Swift (utiliser l'import de données Swift) ; archive pas encore téléchargée d'iCloud : attente jusqu'à 2 minutes ; images des favoris limitées à ce qui est dans les dossiers d'images connus.

## Version de test (DMG)
- `./scripts/make-tauri-test-dmg.sh` : app universelle « Flux Test » (identifiant `io.github.aervorn.flux.test`), signée « Aervorn Code Signing », DMG dans `dist/`. Installable à côté de l'app finale ; ses données sont séparées.

## Release
- Clé de mise à jour (une fois) : `./scripts/setup-updater-key.sh` → `~/.tauri/flux-updater.key` (à sauvegarder, avec son mot de passe) et clé publique dans `tauri.conf.json`.
- `./scripts/make-dmg.sh` : build universel signé (`src-tauri/tauri.release.conf.json`), DMG, `dist/latest.json` (updater Tauri), `appcast.xml` (Sparkle, pour les utilisateurs de l'app Swift ≤ 0.6). Le numéro de build (`bundleVersion` dans `tauri.conf.json`) doit toujours augmenter et rester numérique : Sparkle le compare à celui de l'app Swift installée (6).
- `src-tauri/Info.plist` contient la clé publique Sparkle de l'app Swift : c'est ce qui permet à Sparkle d'accepter le DMG Tauri comme mise à jour. Non testé de bout en bout avant la première release.

## Écarts avec l'app Swift
Aucun écart de fonctions connu : statistiques, hors ligne, archive, raccourcis à une touche, sélection multiple, changement d'adresse, visite guidée et comparaison par le sens sont là. Restent les limites listées plus haut (préchargement limité à 400 articles, etc.) et, côté clavier, des raccourcis lus par lettre (`e.key`) pour ⌘A à cause de l'AZERTY.
