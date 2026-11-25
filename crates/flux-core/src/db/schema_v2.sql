-- Schéma v2 : dossiers intelligents.
-- Un dossier intelligent range les articles dont le titre contient l'un de ses mots-clés ;
-- l'appartenance est recalculée par `smart::rebuild` (après chaque arrivée d'articles ou changement de règle).

CREATE TABLE smart_folders (
    id         INTEGER PRIMARY KEY,
    uid        TEXT NOT NULL UNIQUE,
    name       TEXT NOT NULL,
    whole_word INTEGER NOT NULL DEFAULT 1,
    symbol     TEXT,
    color_name TEXT,
    created_at REAL NOT NULL
);

CREATE TABLE smart_folder_keywords (
    folder_id INTEGER NOT NULL REFERENCES smart_folders(id) ON DELETE CASCADE,
    keyword   TEXT NOT NULL,
    PRIMARY KEY (folder_id, keyword)
);

-- Articles rangés dans un dossier intelligent (un article peut être dans plusieurs dossiers).
CREATE TABLE smart_folder_articles (
    folder_id  INTEGER NOT NULL REFERENCES smart_folders(id) ON DELETE CASCADE,
    article_id INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    PRIMARY KEY (folder_id, article_id)
);
CREATE INDEX smart_folder_articles_article ON smart_folder_articles(article_id);
