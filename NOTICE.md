# Third-party components / Composants tiers

Flux itself is released under the [MIT License](LICENSE). It bundles or uses the following components.
Flux est distribué sous licence MIT ; il embarque ou utilise les composants suivants.

## Fonts / Polices

Bundled through the [Fontsource](https://fontsource.org) packages, under the **SIL Open Font License 1.1** (https://openfontlicense.org). The fonts are used as published (Latin subset, no modification of the designs); each package ships its own license file with the copyright notice of its authors.

| Font | Authors | Package |
|---|---|---|
| Geist, Geist Mono | Vercel, Basement Studio | `@fontsource-variable/geist`, `@fontsource-variable/geist-mono` |
| Inter | Rasmus Andersson | `@fontsource-variable/inter` |
| Literata | TypeTogether, Google | `@fontsource-variable/literata` |
| Merriweather | Sorkin Type | `@fontsource-variable/merriweather` |
| Source Serif 4 | Adobe | `@fontsource-variable/source-serif-4` |
| Atkinson Hyperlegible | Braille Institute of America | `@fontsource/atkinson-hyperlegible` |

## Libraries / Bibliothèques

| Component | License |
|---|---|
| [Readability.js](https://github.com/mozilla/readability) (Mozilla): article extraction | Apache 2.0 |
| [DOMPurify](https://github.com/cure53/DOMPurify): HTML sanitizing | Apache 2.0 or MPL 2.0 |
| [Tauri](https://tauri.app) and its plugins | MIT or Apache 2.0 |
| [React](https://react.dev), [i18next](https://www.i18next.com) | MIT |
| [Lucide](https://lucide.dev) icons | ISC |
| [SQLite](https://sqlite.org) through [rusqlite](https://github.com/rusqlite/rusqlite) | Public domain / MIT |
| Other Rust crates | See `Cargo.lock`; MIT, Apache 2.0 or similar |
| [Sparkle](https://sparkle-project.org) (only in the former Swift app, `swift/`) | MIT |

The Readability.js license text for the Swift app is in `swift/Flux/Resources/Readability-LICENSE.md`.
