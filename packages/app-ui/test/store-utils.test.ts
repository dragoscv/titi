import { describe, expect, it, vi } from "vitest";
import { activeGroup, DEFAULT_RELAY, defaultSettings, useStore, type GroupState } from "../src/store";
import { cn, hueColor, initials } from "../src/cn";
import { host, installHost, type TitiHost } from "../src/platform";

const gs = (id: string): GroupState => ({ id, name: id, fullDuplex: false, memberCount: 1, isActive: false, isCreator: false, members: [], floor: "idle", talkerName: null, talkerNode: null, handover: "stable", link: null, profile: "std", suspended: false, unread: 0 });

describe("store", () => {
  it("starts on the web platform, not ready, with default settings pointing at the relay", () => {
    const s = useStore.getInitialState();
    expect(s).toMatchObject({ ready: false, running: false, platform: "web", voiceSupported: true, nodeId: "", activeGroup: null, levelDbfs: -60, muted: false, pttKeyDown: false, pendingLink: null, pendingRoute: null, toast: null, error: null });
    expect(s.groups).toEqual([]);
    expect(s.settings).toBe(defaultSettings);
    expect(defaultSettings.relayUrl).toBe(DEFAULT_RELAY);
    expect(DEFAULT_RELAY.startsWith("wss://")).toBe(true);
    expect(defaultSettings).toMatchObject({ hue: 40, onboarded: false, volume: 1, lan: true, relay: true, closeToTray: true });
  });

  it("set accepts both a partial object and an updater function", () => {
    useStore.getState().set({ toast: "a" });
    expect(useStore.getState().toast).toBe("a");
    useStore.getState().set((s) => ({ toast: `${s.toast}b` }));
    expect(useStore.getState().toast).toBe("ab");
  });

  it("updateGroup changes only the matching group", () => {
    useStore.setState({ groups: [gs("x"), gs("y")] });
    useStore.getState().updateGroup("y", (g) => ({ ...g, unread: 7 }));
    expect(useStore.getState().groups.map((g) => g.unread)).toEqual([0, 7]);
  });

  it("activeGroup selector returns the active group or null", () => {
    useStore.setState({ groups: [gs("x"), gs("y")], activeGroup: "y" });
    expect(activeGroup(useStore.getState())?.id).toBe("y");
    useStore.setState({ activeGroup: "z" });
    expect(activeGroup(useStore.getState())).toBeNull();
  });
});

describe("cn / hueColor / initials", () => {
  it("cn merges conflicting tailwind classes, last wins, and drops falsy values", () => {
    expect(cn("p-2", false, "p-4", null, "text-sm")).toBe("p-4 text-sm");
  });

  it("hueColor wraps hues into 0..359 including negatives", () => {
    expect(hueColor(40)).toBe("hsl(40 72% 62%)");
    expect(hueColor(400)).toBe("hsl(40 72% 62%)");
    expect(hueColor(-20)).toBe("hsl(340 72% 62%)");
  });

  it("initials takes the first letter of up to two words, or ? for blank names", () => {
    expect(initials("  dragos  catalin vladulescu ")).toBe("DC");
    expect(initials("bob")).toBe("B");
    expect(initials("   ")).toBe("?");
  });
});

describe("platform host facade", () => {
  it("throws a descriptive error before a host is installed", () => {
    expect(() => host.kind).toThrow("titi host not installed (reading kind)");
  });

  it("forwards properties and binds methods to the installed host", () => {
    const impl = { kind: "tv", caps: { lan: false }, n: 0, pttDown: vi.fn(function (this: { n: number }) { this.n++; }) };
    installHost(impl as unknown as TitiHost);
    expect(host.kind).toBe("tv");
    const down = host.pttDown;
    down();
    expect(impl.n).toBe(1);
  });
});
