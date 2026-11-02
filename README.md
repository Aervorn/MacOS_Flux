<p align="center">
  <img src="Design/Flux-icon.svg" width="128" alt="Flux icon" />
</p>

<h1 align="center">Flux</h1>

<p align="center">
  <a href="https://github.com/Aervorn/MacOS_Flux/releases/latest"><img src="https://img.shields.io/github/v/release/Aervorn/MacOS_Flux?label=version&color=FF6A3D" alt="Latest version" /></a>
  <img src="https://img.shields.io/badge/macOS-14%2B-1C1B33?logo=apple&logoColor=white" alt="macOS 14+" />
  <img src="https://img.shields.io/badge/Apple%20Silicon%20%26%20Intel-universal-60A5FA" alt="Universal app" />
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-2DD4BF" alt="MIT License" /></a>
</p>

<p align="center">
  🇬🇧 English · <a href="README.fr.md">🇫🇷 Français</a>
</p>

---

Flux is a simple, good-looking RSS reader for macOS.

You add your feeds yourself (no directory, no account, no recommendations), you browse the articles as a list with thumbnails or as a grid, and you read the **full article right inside the app**, without opening your browser. Everything stays on your Mac.

Flux is a native SwiftUI app, available in English and French, in light and dark mode.

> [!NOTE]
> **Built with the help of AI.** Flux is a personal project I built with [Claude](https://claude.ai), Anthropic's AI assistant: my skills alone wouldn't have been enough to code it entirely by myself. I decided what the app should do, made the design choices and tested every version in daily use; Claude wrote a large part of the code. Just being upfront about it.

## 📸 Screenshots

<p align="center">
  <img src="docs/home_grid_en.png" width="800" alt="Flux main window" />
</p>

| List with thumbnails | Full-width thumbnails |
|:---:|:---:|
| <img src="docs/home_list_en.png" width="400" alt="List with thumbnails" /> | <img src="docs/home_grid-only_en.png" width="400" alt="Full-width thumbnails" /> |
| **Similar articles grouped** | **Reading statistics** |
| <img src="docs/similar_articles_en.png" width="400" alt="Similar articles grouped" /> | <img src="docs/reading_statistics_en.png" width="400" alt="Reading statistics" /> |
| **Reading settings** | **Filter rules** |
| <img src="docs/parameter_reading_en.png" width="300" alt="Reading settings" /> | <img src="docs/exclusion_rules_en.png" width="300" alt="Filter rules" /> |

## ✨ Features

### 📰 Your feeds, your way

- **Add a feed** (⌘N) by pasting its address, or simply the website's address: Flux finds the feed declared by the site, then tries the usual paths (`/feed`, `/rss`, `/atom.xml`…).
- **Folders** (⇧⌘N), collapsible, with their own unread count, color and icon (right-click › Customize…). Drag a feed onto a folder to file it.
- **Rename a feed** (right-click › Rename…) when its name is too long. The original name is kept, so you can go back to it.
- **Change a feed's address** (right-click › Change Address…), for example when a site offers a more complete feed: its articles, read states, favorites, name and folder are kept.
- **OPML import and export** (File menu, ⇧⌘I / ⇧⌘E) to bring your subscriptions from another reader (NetNewsWire, Reeder, Feedly…) with their folders, or to back them up.
- **Limitations:** nested folders in an OPML file are flattened to their closest folder.

### 🖼️ A list you enjoy browsing

- **List** view with a thumbnail, the source, the date, the title and an optional summary, or **Thumbnails** view (a grid of images with the title underneath). Switch between them in the toolbar.
- Thumbnails come from the feed (`media:thumbnail`, `enclosure`…), from the first image of the article, or as a last resort from the page's `og:image`.
- **Full-width thumbnails** (third choice of the layout picker in the toolbar): the grid takes the whole width, and an article opens in its place, with a back arrow (or Esc). Return opens the selected article.
- **Search** in the title and full text of the articles of the current view.
- **Multiple selection** (⌘-click, ⇧-click, ⇧↑/↓, ⌘A) to mark several articles as read, unread or favorite at once.

### 📖 The full article, in the app

- When a feed only provides an excerpt (short, or cut with “…” or “Read more”), Flux downloads the page and extracts the article with [Readability.js](https://github.com/mozilla/readability), the engine behind Firefox's Reader View. A button switches between the feed version and the full article.
- Clean reading layout, light or dark, with an adjustable text size (⌘+ / ⌘− / ⌘0) and three text widths (narrow, wide, full width). Links open in your default browser.
- **Limitations:** articles reserved for a site's subscribers stay cut, since the site itself doesn't serve the rest. Pages built entirely in JavaScript can be extracted poorly: the “Open in Browser” button is there for those.

### 🧲 Similar articles

When several sources publish the same story within 48 hours, Flux groups them: only one article appears in the lists, with a “+N” badge, and the article shows the other sources so you can read their version. Reading it marks the whole group as read.

- Articles are grouped when they point to the same link, or when their titles share at least two distinctive words and are close enough, word for word or in meaning (sentence embeddings from Apple's NaturalLanguage framework, computed on your Mac). Words that appear in many titles at the moment (“iPhone” during a launch week) count very little. A group holds at most one article per source, and an article must resemble the group's first article to join it.
- Settings › Reading: on by default, with three sensitivity levels.
- **Limitations:** grouping is never perfect. It misses some duplicates whose titles are worded very differently, and can occasionally group two different stories: right-click › Separate from Similar Articles. A single feed's view always shows all its articles.

### 🚫 Keyword filters

Rules (Settings › Filters) exclude articles by a keyword in their title, ignoring case and accents: **hide** the article, or simply **mark it as read**. A rule applies to all feeds or only to the feeds you check, to the whole word or anywhere in the title, and shows how many articles it affects (with a preview before you save it).

- Shortcut: right-click a feed › New Filter Rule… opens a rule with that feed already checked.
- Rules apply to existing and incoming articles; favorites are never filtered. Deleting a rule brings back the articles it was hiding.
- **Limitation:** only the title is checked, not the article text.

### ⌨️ Keyboard shortcuts, Reeder style

| Key | Action |
|---|---|
| j / k | Next / previous article |
| Space / ⇧ Space | Scroll the article (at the bottom: next article) |
| m | Mark as read / unread (whole selection) |
| s | Add to / remove from favorites (whole selection) |
| b | Open in browser |
| ↑ / ↓ (← / → in Thumbnails) | Move in the list; with ⇧, extend the selection |
| Return / Esc | Open the article / back to the thumbnails (full-width thumbnails) |
| ⌘N / ⇧⌘N | Add a feed / new folder |
| ⌘R | Refresh |
| ⇧⌘K | Mark all as read (current view) |
| ⌘+ (or ⌘=) / ⌘− / ⌘0 | Bigger / smaller / default text size |
| ⌥⌘E / ⌘P | Export the article as PDF / print it |
| ⌥⌘S | Reading statistics |
| ⌘, | Settings |

Single-key shortcuts are ignored while you type in a text field.

### ✈️ Offline reading

When enabled (Settings › Offline), after each refresh Flux downloads unread articles and favorites in full, **images included**, so you can read them without a connection. Without a connection, the toolbar shows “Offline”.

- The image cache has a size limit (250 MB to 5 GB, 1 GB by default); the least recently used images are removed first. Its size is shown in Settings, with a button to clear it.
- **Limitations:** up to 25 images per article. The first preparation can take a few minutes if you have many unread articles.

### ⭐ Favorites, history and cleanup

- **Favorites** are kept forever, even once read, and are never marked as read automatically.
- **Favorites archive** (Settings › Storage, on by default): when you add a favorite, the full article and its images are stored in a permanent archive, separate from the offline cache. You can read it again even if the site deletes it, or without a connection. Removing a favorite frees its space.
- **Tags** to organize favorites: right-click on articles › Tags, or the tag button in the article toolbar. An article can have several tags, and tagging an article adds it to favorites. Tags appear under Favorites in the sidebar.
- **Export as PDF** (button in the article toolbar, or ⌥⌘E) and **Print** (⌘P), in a light layout made for paper, split into real pages.
- **History** (sidebar) lists the articles you've opened, most recent first, and search works there too.
- **Mark articles as read** immediately, after 5, 10 or 30 seconds, or never (Settings › Reading).
- Optionally **mark old articles as read** after 1 day to 1 month, with a preview of how many articles are affected before you apply.
- Read articles that aren't favorites are **deleted** after 2 weeks to 1 year (2 months by default), or never.
- Deleting a feed deletes its articles **except its favorites**, which stay in Favorites (and go back to the feed if you add it again).

### 📊 Reading statistics

View › Reading Statistics (⌥⌘S): articles read, time spent reading, estimated reading time (at 230 words per minute), a chart of articles read per day, and the most read and most active sources, over 7 days, 30 days, 3 months or 12 months.

- **Limitations:** reading is recorded from this version on (no history before it). An article counts as read after being shown for 3 seconds, and the time spent is the time it stayed on screen, not proof that you read it. Published articles are only counted while Flux keeps them.

### ☁️ Sync between Macs

Settings › iCloud: turn it on on each Mac (same iCloud account) to keep your **feeds, folders, custom names, filter rules, read articles, favorites and their tags** in sync. Flux writes a small state file per Mac in `iCloud Drive/Flux/` and merges the others: when something changed on both Macs, the most recent change wins, item by item.

- Not synced: settings, history, statistics and images (each Mac downloads its own, favorites archive included).
- **Limitations:** iCloud Drive must be turned on; changes arrive as fast as iCloud Drive carries the files (usually within a minute). Flux uses iCloud Drive rather than CloudKit, which requires a paid Apple Developer account. The first time, macOS may ask for permission to access iCloud Drive.

### 💾 Backup to iCloud Drive

Settings › iCloud › Backup: once a day, Flux stores a complete copy of itself (feeds, folders, articles, favorites, tags, rules, history, settings and, optionally, the favorites archive) as a `.zip` in `iCloud Drive/Flux/Sauvegardes/<Mac name>/`, keeping the last 3 to 30. Unlike sync, deleting something doesn't touch backups already made.

- **Restore…** relaunches Flux with the contents of a backup; the current contents are set aside first. If sync is on, the restored contents win over the other Macs and are sent to them.
- Off by default. The image cache isn't backed up: it downloads again.

### ⚙️ Settings

Settings are organized in five tabs (General, Reading, Filters, Storage, iCloud): appearance (automatic, light, dark), language (system, English, French), summary under the title, article text width, mark-as-read delay, refresh interval (15 minutes to 6 hours), offline reading and storage.

## 🧩 Compatibility

**macOS 14 (Sonoma) or later**, on Apple Silicon and Intel Macs.

Feed formats: **RSS 2.0**, **RSS 1.0 (RDF)**, **Atom** and **JSON Feed**, served over `https` or `http`.

## 🔐 Required permissions

None. Flux needs no special permission: it only uses the network to download your feeds, their articles and their images.

## 🔒 Privacy

Flux collects nothing and sends nothing about you. There is no account, no analytics and no tracking. Apart from the optional sync through your own iCloud Drive, the only network requests go directly to the sites you subscribe to: their feeds, the pages of their articles, their images and their favicons. Everything is stored on your Mac:

- articles, feeds and folders: `~/Library/Application Support/Flux/Flux.store`;
- image cache: `~/Library/Application Support/Flux/Images`;
- favorites archive: `~/Library/Application Support/Flux/Archive`;
- sync (if turned on): one file per Mac in `iCloud Drive/Flux/devices/`, stored in your own iCloud.

## 📥 Installation

1. Download `Flux-x.y.z.dmg` from the [latest release](https://github.com/Aervorn/MacOS_Flux/releases/latest).
2. Open it and drag **Flux** onto the **Applications** folder.
3. Launch Flux from your Applications folder.

> [!WARNING]
> **Flux isn't notarized by Apple.** It's signed with the author's own certificate, not with a paid Apple Developer account, so macOS blocks it the first time it's opened. This is expected; you only need to allow it once:
>
> 1. Open Flux: macOS says it can't verify it. Click **Done**.
> 2. Open **System Settings › Privacy & Security** and scroll down to the **Security** section.
> 3. Next to the message about Flux, click **Open Anyway**, then confirm with your password.
>
> On macOS 14 (Sonoma) you can instead Control-click the app and choose **Open**.
>
> If you prefer the Terminal: `xattr -dr com.apple.quarantine /Applications/Flux.app`
>
> You can check that the download is genuine by comparing its SHA-256 checksum (`shasum -a 256 Flux-x.y.z.dmg`) with the one given in the release notes.

## 🔄 Updating

Since version 0.4.0, Flux updates itself: it checks for a new version once a day and offers to install it (or use **Flux › Check for Updates…**). Updates are signed, and Flux refuses any file that wasn't signed by the author. Automatic checking and installing can be turned off in **Settings › General**.

Coming from 0.3.x? Download the new DMG from the [releases page](https://github.com/Aervorn/MacOS_Flux/releases) one last time and replace the app in your Applications folder. Your feeds, articles, favorites and settings are kept.

## 📝 Changelog

### 0.5.0
- **Tags for favorites**: right-click on articles › Tags, or the tag button in the article toolbar. Tags appear under Favorites in the sidebar.
- **Filter rules on several feeds**: check the feeds a rule applies to. Shortcut: right-click a feed › New Filter Rule….
- **Backup to iCloud Drive**: a complete daily copy of Flux, with one-click restore (**Settings › iCloud**).
- **Export as PDF and print** an article (toolbar button, ⌥⌘E, ⌘P).
- **Change a feed's address** without losing its articles (right-click › Change Address…).
- A short tour of Flux on first launch, and a "What's New" window after updates (both in the **Help** menu).
- Clearer context menus in the sidebar, and the General settings tab no longer needs scrolling.

### 0.4.0
- Automatic updates with [Sparkle](https://sparkle-project.org): Flux checks for new versions once a day (**Flux › Check for Updates…**, options in **Settings › General**).
- "Hide read articles" button in the toolbar (also in the **View** menu), for All Articles, folders and feeds.
- The **Add Feed**, **New Folder** and **Refresh** buttons are now in the sidebar.

### 0.3.1
- New signing certificate and bundle identifier. Your feeds, articles and favorites are kept; settings are reset.

### 0.3.0
- First downloadable version (DMG).
- Sync between Macs through iCloud Drive: feeds, folders, names, filter rules, read articles and favorites.
- Adjustable article text size (⌘+ or ⌘= / ⌘− / ⌘0).
- "Fetch Older Articles" for feeds that only publish their last few items (WordPress sites).
- Favorites are kept when their feed is deleted.
- More reliable grouping of similar articles, settings window that fits its content.

### 0.2
- JSON Feed support, full-width thumbnails, keyword filters, favorites archive and History, reading statistics, grouping of similar articles from several sources, settings in tabs.

### 0.1
- First version: manual feeds and folders, list and thumbnail views, full article reading with Readability, search, multiple selection, Reeder-style shortcuts, offline reading, OPML import/export, settings, English and French, light and dark mode.

## 🛠️ Building from source

Requirements: Xcode 15 or later, macOS 14 or later, and [XcodeGen](https://github.com/yonaskolb/XcodeGen) (the Xcode project is generated from `project.yml`).

```bash
brew install xcodegen
git clone https://github.com/Aervorn/MacOS_Flux.git
cd MacOS_Flux
xcodegen generate
open Flux.xcodeproj
```

Choose the **Flux** scheme and **My Mac**, then **Product › Run** (⌘R). The project is set to **Sign to Run Locally**: no Apple developer account is needed.

After a `git pull` that adds or removes files, run `xcodegen generate` again.

```
Flux/
├── FluxApp.swift            entry point, SwiftData database, windows and menus
├── Models.swift             Feed, Folder, Article, FilterRule
├── FeedParser.swift         RSS / Atom / JSON Feed parser and dates
├── FeedStore.swift          adding, refreshing, offline preparation, OPML, cleanup
├── ArticleExtractor.swift   Readability.js in a hidden WKWebView
├── ImageCache.swift         disk image cache, flux-image:// protocol, thumbnails
├── HTMLUtils.swift          plain text, entities, images, feed discovery
├── OPML.swift               OPML import / export
├── Filters.swift            keyword filter rules
├── DuplicateDetector.swift  grouping of similar articles
├── Sync.swift               sync between Macs through iCloud Drive
├── Net.swift                network session
├── Keyboard.swift           single-key shortcuts
├── Settings.swift           settings, appearance, language
├── Updater.swift            automatic updates (Sparkle)
├── Views/                   SwiftUI interface
├── en.lproj / fr.lproj      translations
└── Resources/Readability.js
Design/                      app icon (SVG, and the Python script that generates the PNGs), DMG background
scripts/make-dmg.sh          builds the DMG of a release and the Sparkle appcast
scripts/sparkle-keys.sh      creates the Sparkle signing key (once)
```

To build a DMG like the published ones, `scripts/make-dmg.sh` generates the project, archives the app, checks its signature and creates the disk image. It needs [create-dmg](https://github.com/create-dmg/create-dmg) (`brew install create-dmg`) and a signing certificate, whose name is set at the top of the script. It also signs the DMG for Sparkle and rewrites `appcast.xml`, which needs the EdDSA key created once by `scripts/sparkle-keys.sh` (the private key stays in your keychain).

## 🤝 Contributing & issues

Bug reports, ideas and pull requests are welcome!

- **A feed that doesn't load properly?** [Open an issue](https://github.com/Aervorn/MacOS_Flux/issues/new) with the feed's address, your macOS version, what you expected and what happened. A screenshot helps a lot.
- **An idea?** Open an issue to discuss it first.
- **A pull request?** Keep it focused on one change. The interface texts are written in French in the code; add the English translation of any new text in `Flux/en.lproj/Localizable.strings`.

## ⚠️ Disclaimer

Flux is provided **“as is”, without any warranty**. The author can't be held responsible for any data loss or other damage. Use it at your own risk.

Flux isn't affiliated with the sites whose feeds you read. Articles, images and logos belong to their respective authors.

## 📄 License

Flux is released under the [MIT License](LICENSE).

Article extraction uses [Readability.js](https://github.com/mozilla/readability) by Mozilla, released under the Apache 2.0 License (see `Flux/Resources/Readability-LICENSE.md`).
