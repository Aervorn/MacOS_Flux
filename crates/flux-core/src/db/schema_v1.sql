-- Schéma v1 de la base Flux (SQLite).
-- Les dates sont des secondes Unix (REAL). Les booléens sont 0/1.

CREATE TABLE folders (
    id          INTEGER PRIMARY KEY,
    uid         TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    is_expanded INTEGER NOT NULL DEFAULT 1,
    created_at  REAL NOT NULL,
    symbol      TEXT,
    color_name  TEXT
);

CREATE TABLE feeds (
    id             INTEGER PRIMARY KEY,
    url            TEXT NOT NULL UNIQUE,
    title          TEXT NOT NULL,
    site_url       TEXT,
    added_at       REAL NOT NULL,
    last_refreshed REAL,
    last_error     TEXT,
    folder_id      INTEGER REFERENCES folders(id) ON DELETE SET NULL,
    custom_title   TEXT,
    previous_url   TEXT
);
CREATE INDEX feeds_folder ON feeds(folder_id);

CREATE TABLE articles (
    id              INTEGER PRIMARY KEY,
    key             TEXT NOT NULL UNIQUE,   -- feedURL + "|" + guid (ou lien, ou titre)
    feed_id         INTEGER REFERENCES feeds(id) ON DELETE SET NULL,
    feed_url        TEXT NOT NULL,
    title           TEXT NOT NULL,
    link            TEXT,
    summary         TEXT NOT NULL DEFAULT '',
    content_html    TEXT,
    full_html       TEXT,
    search_text     TEXT NOT NULL DEFAULT '',
    is_hidden       INTEGER NOT NULL DEFAULT 0,
    filtered_by     TEXT,                   -- filter_rules.uid
    is_archived     INTEGER NOT NULL DEFAULT 0,
    opened_at       REAL,
    reading_seconds REAL NOT NULL DEFAULT 0,
    word_count      INTEGER NOT NULL DEFAULT 0,
    cluster_id      TEXT,
    is_duplicate    INTEGER NOT NULL DEFAULT 0,
    cluster_opt_out INTEGER NOT NULL DEFAULT 0,
    source_title    TEXT,
    image_url       TEXT,
    author          TEXT,
    published       REAL NOT NULL,
    is_read         INTEGER NOT NULL DEFAULT 0,
    is_starred      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX articles_feed ON articles(feed_id);
CREATE INDEX articles_published ON articles(published);
CREATE INDEX articles_cluster ON articles(cluster_id);

CREATE TABLE favorite_tags (
    id         INTEGER PRIMARY KEY,
    uid        TEXT NOT NULL UNIQUE,
    name       TEXT NOT NULL,
    created_at REAL NOT NULL
);

CREATE TABLE article_tags (
    article_id INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    tag_id     INTEGER NOT NULL REFERENCES favorite_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (article_id, tag_id)
);
CREATE INDEX article_tags_tag ON article_tags(tag_id);

CREATE TABLE filter_rules (
    id         INTEGER PRIMARY KEY,
    uid        TEXT NOT NULL UNIQUE,
    keyword    TEXT NOT NULL,
    whole_word INTEGER NOT NULL DEFAULT 1,
    action     TEXT NOT NULL,               -- "hide" ou "markRead"
    created_at REAL NOT NULL
);

-- Flux concernés par une règle (aucune ligne = tous les flux).
-- On garde l'adresse du flux (et non son id) : une règle peut viser un flux supprimé ou pas encore importé.
CREATE TABLE filter_rule_feeds (
    rule_id  INTEGER NOT NULL REFERENCES filter_rules(id) ON DELETE CASCADE,
    feed_url TEXT NOT NULL,
    PRIMARY KEY (rule_id, feed_url)
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL                      -- JSON
);
