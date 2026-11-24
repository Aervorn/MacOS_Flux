import { describe, expect, it } from "vitest";
import { extractArticle, firstUsableImage } from "./extract";
import { bodyHtml, heroAlreadyInBody, sanitizeArticle } from "./sanitize";
import { buildSearchText, looksLikeExcerpt, plainText, wordCount } from "./text";

describe("sanitizeArticle", () => {
  it("removes scripts, event handlers, styles and frames", () => {
    const out = sanitizeArticle(
      '<p onclick="x()">Bonjour</p><script>alert(1)</script><style>p{}</style><iframe src="https://e.com"></iframe><img src="https://e.com/a.jpg" onerror="x()" style="width:1px">',
    );
    expect(out).toContain("Bonjour");
    expect(out).not.toMatch(/script|onclick|onerror|<style|iframe|style=/i);
    expect(out).toContain("https://e.com/a.jpg");
  });

  it("blocks javascript: links and forms", () => {
    const out = sanitizeArticle('<a href="javascript:alert(1)">x</a><form action="https://e.com"><input name="a"></form>');
    expect(out).not.toMatch(/javascript:|<form|<input/i);
  });

  it("drops srcset so the cache rewrite stays in control", () => {
    expect(sanitizeArticle('<img src="https://e.com/a.jpg" srcset="https://e.com/b.jpg 2x">')).not.toContain("srcset");
  });
});

describe("text helpers", () => {
  it("detects excerpts like the Swift app", () => {
    expect(looksLikeExcerpt("<p>Court.</p>")).toBe(true);
    expect(looksLikeExcerpt(null)).toBe(false);
    const long = `<p>${"mot ".repeat(300)}</p>`; // ~1200 caractères
    expect(looksLikeExcerpt(long)).toBe(false);
    expect(looksLikeExcerpt(`<p>${"mot ".repeat(300)}…</p>`)).toBe(true);
    expect(looksLikeExcerpt(`<p>${"mot ".repeat(300)} Lire la suite</p>`)).toBe(true);
    expect(looksLikeExcerpt(`<p>${"mot ".repeat(1500)}</p>`)).toBe(false);
  });

  it("builds plain text, word count and search text", () => {
    expect(plainText("<p>Un <b>deux</b></p><script>x</script>")).toBe("Un deux");
    expect(wordCount(" un  deux\ntrois ")).toBe(3);
    expect(buildSearchText("T", "S", "B")).toBe("T\nS\nB");
    expect(buildSearchText("T", "S", "x".repeat(40000)).length).toBe(30000);
  });
});

describe("body and hero", () => {
  it("falls back from full text to feed content to the escaped summary", () => {
    const base = { fullHtml: "<p>F</p>", contentHtml: "<p>C</p>", summary: "a<b", showFull: true };
    expect(bodyHtml(base)).toBe("<p>F</p>");
    expect(bodyHtml({ ...base, showFull: false })).toBe("<p>C</p>");
    expect(bodyHtml({ ...base, showFull: false, contentHtml: "  " })).toBe("<p>a&lt;b</p>");
  });

  it("skips the hero image when the body already has it", () => {
    expect(heroAlreadyInBody("https://e.com/img/photo.jpg?w=1", '<img src="https://cdn/photo.jpg">')).toBe(true);
    expect(heroAlreadyInBody("https://e.com/other.jpg", '<img src="https://cdn/photo.jpg">')).toBe(false);
  });
});

describe("extractArticle", () => {
  const page = `<html><head><title>T</title></head><body><nav>menu</nav><article><h1>Titre</h1>${"<p>Un paragraphe assez long pour que Readability le garde, avec du texte, des virgules, et encore du texte.</p>".repeat(8)}<img src="/a.jpg"></article></body></html>`;

  it("extracts the content and resolves relative links against the page", () => {
    const r = extractArticle(page, "https://site.example/news/1", "Titre", "Résumé");
    expect(r).not.toBeNull();
    expect(r!.wordCount).toBeGreaterThan(50);
    expect(r!.searchText.startsWith("Titre\nRésumé\n")).toBe(true);
    expect(r!.imageUrl).toBe("https://site.example/a.jpg");
  });

  it("ignores tracking pixels", () => {
    expect(firstUsableImage('<img src="https://t.example/pixel.gif"><img src="https://e.com/ok.jpg">')).toBe("https://e.com/ok.jpg");
    expect(firstUsableImage('<img src="https://e.com/x.jpg" width="1" height="1">')).toBeNull();
  });
});
