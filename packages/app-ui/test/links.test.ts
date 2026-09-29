import { describe, expect, it } from "vitest";
import { WEB_ORIGIN, isInviteUrl, toWebLink } from "../src/links";

const PATH = "AAAA/BBBB/123/CCCC/DDDD";

describe("toWebLink", () => {
  it("rewrites a titi:// deep link to the public web origin", () => {
    expect(toWebLink(`titi://j/${PATH}`)).toBe(`https://titi.dragoscatalin.ro/j/${PATH}`);
    expect(WEB_ORIGIN).toBe("https://titi.dragoscatalin.ro");
  });

  it("leaves non-deep-link strings unchanged", () => {
    expect(toWebLink("hello")).toBe("hello");
  });
});

describe("isInviteUrl", () => {
  it("accepts the new web origin", () => {
    expect(isInviteUrl(`https://titi.dragoscatalin.ro/j/${PATH}`)).toBe(true);
  });

  it("accepts legacy titi.app links from old QR codes", () => {
    expect(isInviteUrl(`https://titi.app/j/${PATH}`)).toBe(true);
  });

  it("accepts titi:// deep links", () => {
    expect(isInviteUrl(`titi://j/${PATH}`)).toBe(true);
  });

  it("rejects unrelated https URLs", () => {
    expect(isInviteUrl("https://example.com/j/x")).toBe(false);
    expect(isInviteUrl("ABCD-1234")).toBe(false);
  });

  it("rejects a lookalike path on another host", () => {
    expect(isInviteUrl("https://evil.com/titi.app/j/x")).toBe(false);
    expect(isInviteUrl("https://evil.com/titi.dragoscatalin.ro/j/x")).toBe(false);
  });
});
