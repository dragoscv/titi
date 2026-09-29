import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyUi, setGroups, type ReducerHooks } from "../src/reducer";
import { defaultSettings, useStore } from "../src/store";
import type { GroupJson } from "../src/types";

const ME = "aa".repeat(8);
const BOB = "bb".repeat(8);
const G = "01".repeat(16);
const group = (members: { node: string; name: string }[]): GroupJson => ({ id: G, name: "Ceas", fullDuplex: false, memberCount: members.length, isActive: true, isCreator: true, members: members.map((m) => ({ ...m, hue: 40 })) });

type Hooks = { cue: ReturnType<typeof vi.fn<ReducerHooks["cue"]>>; refreshGroups: ReturnType<typeof vi.fn<() => void>>; syncRooms: ReturnType<typeof vi.fn<() => void>>; notify: ReturnType<typeof vi.fn<(title: string, body: string, tag: string) => void>> };
let hooks: Hooks;

beforeEach(() => {
  useStore.setState({ nodeId: ME, groups: [], activeGroup: null, peers: {}, invites: [], messages: [], settings: defaultSettings, toast: null });
  hooks = { cue: vi.fn<ReducerHooks["cue"]>(), refreshGroups: vi.fn<() => void>(), syncRooms: vi.fn<() => void>(), notify: vi.fn<(title: string, body: string, tag: string) => void>() };
  setGroups([group([{ node: ME, name: "Me" }, { node: BOB, name: "Bob" }])]);
});

describe("floor events", () => {
  it("marks the group busy with the talker's name when someone else takes the floor, and cues incoming", () => {
    applyUi({ type: "floorTaken", group: G, holder: BOB, name: "Bob", prio: 0 }, hooks);
    const g = useStore.getState().groups[0]!;
    expect(g.floor).toBe("busy");
    expect(g.talkerName).toBe("Bob");
    expect(hooks.cue).toHaveBeenCalledWith("incoming");
  });

  it("plays the released cue only when our own floor goes idle", () => {
    applyUi({ type: "floorGranted" }, hooks);
    expect(useStore.getState().groups[0]!.floor).toBe("talking");
    applyUi({ type: "floorIdle", group: G }, hooks);
    expect(useStore.getState().groups[0]!.floor).toBe("idle");
    expect(hooks.cue).toHaveBeenCalledWith("released");

    hooks.cue.mockClear();
    applyUi({ type: "floorTaken", group: G, holder: BOB, name: "Bob", prio: 0 }, hooks);
    applyUi({ type: "floorIdle", group: G }, hooks);
    expect(hooks.cue).not.toHaveBeenCalledWith("released");
  });
});

describe("messages", () => {
  const text = (id: string, from = BOB) => ({ type: "message" as const, group: G, from, msg_uuid: id, sent_ms: 1, body: { kind: "text" as const, text: "hi" } });

  it("de-duplicates a message delivered twice (LAN + relay)", () => {
    applyUi(text("m1"), hooks);
    applyUi(text("m1"), hooks);
    expect(useStore.getState().messages).toHaveLength(1);
  });

  it("counts unread only for groups that are not active and never for own messages", () => {
    useStore.setState({ activeGroup: null });
    applyUi(text("m1"), hooks);
    applyUi(text("m2", ME), hooks);
    expect(useStore.getState().groups[0]!.unread).toBe(1);
    expect(hooks.notify).toHaveBeenCalledTimes(1);
  });

  it("raises an SOS cue and notification", () => {
    applyUi({ type: "message", group: G, from: BOB, msg_uuid: "s1", sent_ms: 1, body: { kind: "sos", lat_e7: 0, lon_e7: 0, note: "", cancelled: false } }, hooks);
    expect(hooks.cue).toHaveBeenCalledWith("sos");
    expect(hooks.notify.mock.calls[0]![0]).toContain("SOS");
  });
});

describe("groups", () => {
  it("keeps floor/unread state across a group refresh (membership change)", () => {
    applyUi({ type: "floorTaken", group: G, holder: BOB, name: "Bob", prio: 0 }, hooks);
    setGroups([group([{ node: ME, name: "Me" }, { node: BOB, name: "Bob" }, { node: "cc".repeat(8), name: "TV" }])]);
    const g = useStore.getState().groups[0]!;
    expect(g.memberCount).toBe(3);
    expect(g.floor).toBe("busy");
  });

  it("memberJoined asks the host to refresh the roster", () => {
    applyUi({ type: "memberJoined", group: G, node: "cc".repeat(8), name: "TV" }, hooks);
    expect(hooks.refreshGroups).toHaveBeenCalled();
  });

  it("replaces a repeated invite for the same group instead of stacking it", () => {
    const inv = { type: "inviteOffered" as const, group: G, name: "Ceas", host: BOB, host_name: "Bob", members: 2 };
    applyUi(inv, hooks);
    applyUi(inv, hooks);
    expect(useStore.getState().invites).toHaveLength(1);
  });
});
