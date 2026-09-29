import { afterEach, describe, expect, it, vi } from "vitest";
import { loadConfig } from "../src/config.ts";
import { log, setLogLevel } from "../src/log.ts";

describe("loadConfig", () => {
  it("applies documented defaults for an empty environment", () => {
    expect(loadConfig({})).toEqual({
      PORT: 8080,
      MAX_ROOM_SIZE: 64,
      RATE_LIMIT_FPS: 120,
      MAX_FRAME_BYTES: 4096,
      RESUME_TTL_MS: 300_000,
      LOG_LEVEL: "info",
    });
  });

  it("coerces numeric strings and ignores unrelated variables", () => {
    const cfg = loadConfig({ PORT: "9000", MAX_ROOM_SIZE: "8", RESUME_TTL_MS: "1500", LOG_LEVEL: "debug", HOME: "/x" });
    expect(cfg).toMatchObject({ PORT: 9000, MAX_ROOM_SIZE: 8, RESUME_TTL_MS: 1500, LOG_LEVEL: "debug" });
    expect(cfg).not.toHaveProperty("HOME");
  });

  it("reads process.env when called without arguments", () => {
    vi.stubEnv("MAX_FRAME_BYTES", "777");
    try {
      expect(loadConfig().MAX_FRAME_BYTES).toBe(777);
    } finally {
      vi.unstubAllEnvs();
    }
  });

  it.each([
    ["PORT", "abc"],
    ["PORT", "0"],
    ["MAX_ROOM_SIZE", "-1"],
    ["RATE_LIMIT_FPS", "1.5"],
    ["LOG_LEVEL", "verbose"],
  ])("throws on invalid %s=%s", (key, value) => {
    expect(() => loadConfig({ [key]: value })).toThrow(new RegExp(key));
  });
});

function parseLine(raw: string): Record<string, unknown> {
  const parsed: unknown = JSON.parse(raw);
  if (typeof parsed !== "object" || parsed === null) throw new Error(`not a JSON object: ${raw}`);
  return { ...parsed };
}

function capture() {
  const out = vi.spyOn(process.stdout, "write").mockImplementation(() => true);
  const err = vi.spyOn(process.stderr, "write").mockImplementation(() => true);
  const lines = (spy: typeof out) => spy.mock.calls.map((c) => parseLine(String(c[0])));
  return { out: () => lines(out), err: () => lines(err) };
}

describe("log", () => {
  afterEach(() => {
    setLogLevel("info");
    vi.restoreAllMocks();
  });

  it("drops levels below the minimum (default info)", () => {
    const c = capture();
    log.debug("hidden");
    log.info("shown", { room: "01020304" });
    expect(c.out()).toHaveLength(1);
    expect(c.out()[0]).toMatchObject({ severity: "INFO", message: "shown", room: "01020304" });
    expect(typeof c.out()[0]!.time).toBe("string");
  });

  it("debug level emits debug; error level emits only errors, to stderr", () => {
    const c = capture();
    setLogLevel("debug");
    log.debug("d");
    expect(c.out().map((l) => l.severity)).toEqual(["DEBUG"]);
    setLogLevel("error");
    log.info("i");
    log.warn("w");
    log.error("e", { code: 7 });
    expect(c.out()).toHaveLength(1);
    expect(c.err()).toEqual([expect.objectContaining({ severity: "ERROR", message: "e", code: 7 })]);
  });

  it("warn level keeps warn on stdout and filters info", () => {
    const c = capture();
    setLogLevel("warn");
    log.info("i");
    log.warn("w");
    expect(c.out().map((l) => l.message)).toEqual(["w"]);
    expect(c.err()).toHaveLength(0);
  });
});
