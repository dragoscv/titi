import { afterEach, describe, expect, it } from "vitest";
import { detectLocale, dictionaries, locale, setLocale, t } from "../src/i18n";

afterEach(() => setLocale(null));

describe("i18n", () => {
  it("EN and RO have exactly the same keys", () => {
    expect(Object.keys(dictionaries.ro).sort()).toEqual(Object.keys(dictionaries.en).sort());
  });

  it("every string is non-empty and uses the same placeholders in both locales", () => {
    const vars = (s: string) => (s.match(/\{\w+\}/g) ?? []).sort();
    for (const k of Object.keys(dictionaries.en) as (keyof typeof dictionaries.en)[]) {
      expect(dictionaries.en[k].trim(), k).not.toBe("");
      expect(dictionaries.ro[k].trim(), k).not.toBe("");
      expect(vars(dictionaries.ro[k]), k).toEqual(vars(dictionaries.en[k]));
    }
  });

  it("Romanian uses comma-below ș ț, never the cedilla forms ş ţ", () => {
    const all = Object.values(dictionaries.ro).join(" ");
    expect(all).not.toMatch(/[şţŞŢ]/);
    expect(all).toMatch(/[șț]/);
  });

  it("detects Romanian from any ro* language tag, English otherwise", () => {
    expect(detectLocale("ro-RO")).toBe("ro");
    expect(detectLocale("RO")).toBe("ro");
    expect(detectLocale("en-GB")).toBe("en");
    expect(detectLocale(undefined)).toBe("en");
  });

  it("an explicit locale wins over the system language", () => {
    setLocale("ro");
    expect(locale()).toBe("ro");
    expect(t("chat.deleted")).toBe("Acest mesaj a fost șters");
    setLocale("en");
    expect(t("chat.deleted")).toBe("This message was deleted");
  });

  it("interpolates variables and leaves unknown placeholders alone", () => {
    setLocale("en");
    expect(t("chat.reactionMine", { emoji: "👍", count: 2 })).toBe("👍 2, including you");
    expect(t("chat.replyingTo")).toBe("Replying to {name}");
    expect(t("chat.replyingTo", { other: 1 })).toBe("Replying to {name}");
  });
});
