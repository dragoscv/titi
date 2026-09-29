import { beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import { applyUi, nameOf, setGroups, type ReducerHooks } from "../src/reducer";
import { defaultSettings, useStore } from "../src/store";
import type { GroupJson, MsgBody } from "../src/types";

const ME = "aa".repeat(8);
const BOB = "bb".repeat(8);
const CAT = "cc".repeat(8);
const G = "01".repeat(16);
const G2 = "02".repeat(16);
const group = (id: string, active: boolean): GroupJson => ({
  id,
  name: id === G ? "Ceas" : "Munte",
  fullDuplex: false,
  memberCount: 2,
  isActive: active,
  isCreator: true,
  members: [{ node: ME, name: "Me", hue: 10 }, { node: BOB, name: "Bob", hue: 200 }],
});

type Hooks = { cue: Mock<ReducerHooks["cue"]>; refreshGroups: Mock<() => void>; syncRooms: Mock<() => void>; notify: Mock<(title: string, body: string, tag: string) => void> };
let hooks: Hooks;
const st = () => useStore.getState();
const g1 = () => st().groups.find((g) => g.id === G)!;
const g2 = () => st().groups.find((g) => g.id === G2)!;

beforeEach(() => {
  useStore.setState({ nodeId: ME, groups: [], activeGroup: null, peers: {}, invites: [], messages: [], settings: defaultSettings, toast: null, levelDbfs: -60 });
  hooks = { cue: vi.fn<ReducerHooks["cue"]>(), refreshGroups: vi.fn<() => void>(), syncRooms: vi.fn<() => void>(), notify: vi.fn<(title: string, body: string, tag: string) => void>() };
  setGroups([group(G, true), group(G2, false)]);
});

describe("setGroups", () => {
  it("selects the group flagged active and starts new groups in a fresh idle state", () => {
    expect(st().activeGroup).toBe(G);
    expect(g2()).toMatchObject({ floor: "idle", talkerName: null, talkerNode: null, handover: "stable", link: null, profile: "std", suspended: false, unread: 0 });
  });

  it("clears the active group when no group is active", () => {
    setGroups([group(G, false)]);
    expect(st().activeGroup).toBeNull();
    expect(st().groups).toHaveLength(1);
  });

  it("learns member names so nameOf resolves them, falling back to a 6-char node prefix", () => {
    expect(nameOf(BOB)).toBe("Bob");
    expect(nameOf("0123456789abcdef")).toBe("012345");
  });
});

describe("peer events", () => {
  it("peerDiscovered adds a peer with zero bars/hops and records its name", () => {
    applyUi({ type: "peerDiscovered", node: CAT, name: "Cat", hue: 120, link: "internet", in_group: false }, hooks);
    expect(st().peers[CAT]).toEqual({ node: CAT, name: "Cat", hue: 120, link: "internet", inGroup: false, bars: 0, hops: 0 });
    expect(nameOf(CAT)).toBe("Cat");
  });

  it("peerDiscovered again keeps existing bars/hops but updates name and link", () => {
    applyUi({ type: "peerDiscovered", node: CAT, name: "Cat", hue: 120, link: "internet", in_group: false }, hooks);
    applyUi({ type: "peerLink", node: CAT, link: "internet", bars: 3, hops: 2 }, hooks);
    applyUi({ type: "peerDiscovered", node: CAT, name: "Cathy", hue: 121, link: "lan", in_group: true }, hooks);
    expect(st().peers[CAT]).toEqual({ node: CAT, name: "Cathy", hue: 121, link: "lan", inGroup: true, bars: 3, hops: 2 });
  });

  it("peerLink updates link quality of a known peer and ignores unknown peers", () => {
    applyUi({ type: "peerDiscovered", node: CAT, name: "Cat", hue: 1, link: "internet", in_group: true }, hooks);
    applyUi({ type: "peerLink", node: CAT, link: "webRtc", bars: 4, hops: 1 }, hooks);
    applyUi({ type: "peerLink", node: BOB, link: "lan", bars: 1, hops: 1 }, hooks);
    expect(st().peers[CAT]).toMatchObject({ link: "webRtc", bars: 4, hops: 1 });
    expect(st().peers[BOB]).toBeUndefined();
  });

  it("peerLost removes only that peer", () => {
    applyUi({ type: "peerDiscovered", node: CAT, name: "Cat", hue: 1, link: "internet", in_group: true }, hooks);
    applyUi({ type: "peerDiscovered", node: BOB, name: "Bob", hue: 1, link: "internet", in_group: true }, hooks);
    applyUi({ type: "peerLost", node: CAT }, hooks);
    expect(Object.keys(st().peers)).toEqual([BOB]);
  });
});

describe("floor events", () => {
  it("floorGranted marks the active group talking by me and cues granted", () => {
    applyUi({ type: "floorGranted" }, hooks);
    expect(g1()).toMatchObject({ floor: "talking", talkerName: null, talkerNode: ME });
    expect(g2().floor).toBe("idle");
    expect(hooks.cue).toHaveBeenCalledWith("granted");
  });

  it("floorDenied marks the active group busy and cues denied", () => {
    applyUi({ type: "floorDenied", holder: BOB }, hooks);
    expect(g1().floor).toBe("busy");
    expect(hooks.cue).toHaveBeenCalledWith("denied");
  });

  it("floor events for the active group are dropped when there is no active group", () => {
    useStore.setState({ activeGroup: null });
    applyUi({ type: "floorGranted" }, hooks);
    expect(g1().floor).toBe("idle");
    expect(hooks.cue).toHaveBeenCalledWith("granted");
  });

  it("floorTaken by me (other device echo) shows talking and does not cue incoming", () => {
    applyUi({ type: "floorTaken", group: G2, holder: ME, name: "Me", prio: 0 }, hooks);
    expect(g2()).toMatchObject({ floor: "talking", talkerName: "Me", talkerNode: ME });
    expect(hooks.cue).not.toHaveBeenCalled();
  });

  it("floorIdle clears the talker of that group", () => {
    applyUi({ type: "floorTaken", group: G2, holder: BOB, name: "Bob", prio: 1 }, hooks);
    applyUi({ type: "floorIdle", group: G2 }, hooks);
    expect(g2()).toMatchObject({ floor: "idle", talkerName: null, talkerNode: null });
  });

  it("talkWarning toasts the remaining time and cues warning", () => {
    applyUi({ type: "talkWarning" }, hooks);
    expect(st().toast).toBe("10 seconds left");
    expect(hooks.cue).toHaveBeenCalledWith("warning");
  });

  it("talkTimeout toasts the limit without a cue", () => {
    applyUi({ type: "talkTimeout" }, hooks);
    expect(st().toast).toBe("Talk time limit reached");
    expect(hooks.cue).not.toHaveBeenCalled();
  });
});

describe("invites and membership", () => {
  it("inviteOffered stores the invite, cues incoming and notifies with host and member count", () => {
    applyUi({ type: "inviteOffered", group: G2, name: "Munte", host: BOB, host_name: "Bob", members: 3 }, hooks);
    expect(st().invites).toEqual([{ group: G2, name: "Munte", host: BOB, hostName: "Bob", members: 3 }]);
    expect(hooks.cue).toHaveBeenCalledWith("incoming");
    expect(hooks.notify).toHaveBeenCalledWith("Bob invites you", "Join “Munte” · 3 members", `invite-${G2}`);
  });

  it("inviteOffered works without an optional notify hook", () => {
    const { notify: _n, ...noNotify } = hooks;
    applyUi({ type: "inviteOffered", group: G2, name: "Munte", host: BOB, host_name: "Bob", members: 3 }, noNotify);
    expect(st().invites).toHaveLength(1);
  });

  it("joined refreshes groups, re-syncs relay rooms and toasts the group name", () => {
    applyUi({ type: "joined", group: G2, name: "Munte" }, hooks);
    expect(hooks.refreshGroups).toHaveBeenCalledTimes(1);
    expect(hooks.syncRooms).toHaveBeenCalledTimes(1);
    expect(st().toast).toBe("Joined Munte");
  });

  it("joinFailed toasts the reason", () => {
    applyUi({ type: "joinFailed", reason: "code expired" }, hooks);
    expect(st().toast).toBe("Could not join: code expired");
  });

  it("memberJoined records the new member's name", () => {
    applyUi({ type: "memberJoined", group: G, node: "dd".repeat(8), name: "Dan" }, hooks);
    expect(nameOf("dd".repeat(8))).toBe("Dan");
    expect(hooks.refreshGroups).toHaveBeenCalledTimes(1);
  });

  it("memberLeft refreshes the roster", () => {
    applyUi({ type: "memberLeft", group: G, node: BOB }, hooks);
    expect(hooks.refreshGroups).toHaveBeenCalledTimes(1);
  });
});

describe("messages", () => {
  const msg = (id: string, body: MsgBody, from = BOB, grp = G) => ({ type: "message" as const, group: grp, from, msg_uuid: id, sent_ms: 1234, body });

  it("stores a text message with sender name, mine=false, acked=false and notifies", () => {
    applyUi(msg("m1", { kind: "text", text: "salut" }), hooks);
    expect(st().messages).toEqual([{ id: "m1", group: G, from: BOB, fromName: "Bob", sentMs: 1234, body: { kind: "text", text: "salut" }, mine: false, acked: false }]);
    expect(hooks.notify).toHaveBeenCalledWith("Bob", "salut", `msg-${G}`);
    expect(g1().unread).toBe(0);
  });

  it("counts unread on a background group", () => {
    applyUi(msg("m1", { kind: "text", text: "a" }, BOB, G2), hooks);
    applyUi(msg("m2", { kind: "text", text: "b" }, BOB, G2), hooks);
    expect(g2().unread).toBe(2);
  });

  it("own messages are marked mine and never notify or cue", () => {
    applyUi(msg("m1", { kind: "sos", lat_e7: 0, lon_e7: 0, note: "", cancelled: false }, ME), hooks);
    expect(st().messages[0]!.mine).toBe(true);
    expect(hooks.notify).not.toHaveBeenCalled();
    expect(hooks.cue).not.toHaveBeenCalled();
  });

  it("a cancelled SOS notifies that the sender is safe, with the note as body", () => {
    applyUi(msg("s1", { kind: "sos", lat_e7: 1, lon_e7: 2, note: "ok now", cancelled: true }), hooks);
    expect(hooks.cue).toHaveBeenCalledWith("sos");
    expect(hooks.notify).toHaveBeenCalledWith("Bob is safe", "ok now", `sos-${G}`);
  });

  it("an SOS without a note says Emergency", () => {
    applyUi(msg("s1", { kind: "sos", lat_e7: 1, lon_e7: 2, note: "", cancelled: false }), hooks);
    expect(hooks.notify).toHaveBeenCalledWith("SOS from Bob", "Emergency", `sos-${G}`);
  });

  it("voice notes and locations are stored without a notification", () => {
    applyUi(msg("v1", { kind: "voiceNote", profile: "std", duration_ms: 900, opus_packets: [] }), hooks);
    applyUi(msg("l1", { kind: "location", lat_e7: 1, lon_e7: 2, accuracy_m: 5, breadcrumb: false }), hooks);
    expect(st().messages.map((m) => m.id)).toEqual(["v1", "l1"]);
    expect(hooks.notify).not.toHaveBeenCalled();
  });

  it("keeps only the newest 500 messages", () => {
    for (let i = 0; i < 502; i++) applyUi(msg(`m${i}`, { kind: "location", lat_e7: 0, lon_e7: 0, accuracy_m: 1, breadcrumb: true }), hooks);
    expect(st().messages).toHaveLength(500);
    expect(st().messages[0]!.id).toBe("m2");
    expect(st().messages[499]!.id).toBe("m501");
  });

  it("messageAcked flips acked only on the matching message", () => {
    applyUi(msg("m1", { kind: "text", text: "a" }, ME), hooks);
    applyUi(msg("m2", { kind: "text", text: "b" }, ME), hooks);
    applyUi({ type: "messageAcked", msg_uuid: "m2", by: BOB }, hooks);
    expect(st().messages.map((m) => m.acked)).toEqual([false, true]);
  });
});

describe("group state events", () => {
  it("handover stores state, link and profile", () => {
    applyUi({ type: "handover", group: G, state: "migrating", link: "internet", profile: "low" }, hooks);
    expect(g1()).toMatchObject({ handover: "migrating", link: "internet", profile: "low" });
  });

  it("suspended and resumed toggle the suspended flag", () => {
    applyUi({ type: "suspended", group: G2 }, hooks);
    expect(g2().suspended).toBe(true);
    applyUi({ type: "resumed", group: G2 }, hooks);
    expect(g2().suspended).toBe(false);
  });

  it("modeChanged sets full duplex", () => {
    applyUi({ type: "modeChanged", group: G, full_duplex: true }, hooks);
    expect(g1().fullDuplex).toBe(true);
  });

  it("level updates the meter for other talkers and for silence, but not for my own voice", () => {
    applyUi({ type: "level", talker: BOB, dbfs: -12 }, hooks);
    expect(st().levelDbfs).toBe(-12);
    applyUi({ type: "level", talker: ME, dbfs: -3 }, hooks);
    expect(st().levelDbfs).toBe(-12);
    applyUi({ type: "level", talker: null, dbfs: -50 }, hooks);
    expect(st().levelDbfs).toBe(-50);
  });

  it("error toasts the engine message", () => {
    applyUi({ type: "error", message: "no such group" }, hooks);
    expect(st().toast).toBe("no such group");
  });
});
