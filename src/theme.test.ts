import { describe, expect, it } from "vitest";
import { resolveTheme, schemeOf, type ThemeChoice } from "./theme";

const auto: ThemeChoice = { appearance: "system", lightTheme: "sepia", darkTheme: "nord", accent: "theme" };

describe("choix du thème", () => {
  it("suit le thème de jour et le thème de nuit en automatique", () => {
    expect(resolveTheme(auto, false)).toBe("sepia");
    expect(resolveTheme(auto, true)).toBe("nord");
  });

  it("garde un thème précis quelle que soit l'apparence de macOS", () => {
    expect(resolveTheme({ ...auto, appearance: "dawn" }, true)).toBe("dawn");
    expect(resolveTheme({ ...auto, appearance: "forest" }, false)).toBe("forest");
  });

  it("retombe sur Clair ou Sombre pour une valeur inconnue", () => {
    const odd = { ...auto, lightTheme: "?", darkTheme: "?", appearance: "?" } as unknown as ThemeChoice;
    expect(resolveTheme(odd, false)).toBe("light");
    expect(resolveTheme(odd, true)).toBe("dark");
  });

  it("distingue les thèmes clairs des thèmes sombres", () => {
    expect(schemeOf("sepia")).toBe("light");
    expect(schemeOf("pureBlack")).toBe("dark");
  });
});
