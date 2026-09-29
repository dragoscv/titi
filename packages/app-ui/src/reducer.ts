// Applies engine UiEvents to the store. Shared by every host so web, desktop
// and TV render the exact same state for the same event stream.
import { useStore, type GroupState, type Msg } from "./store";
import type { GroupJson, UiEvent } from "./types";

export type CueKind = "granted" | "released" | "denied" | "incoming" | "warning" | "sos";

export interface ReducerHooks {
  cue(kind: CueKind): void;
  refreshGroups(): void;
  syncRooms(): void;
  /** Optional system notification (desktop toasts, TV banner). */
  notify?(title: string, body: string, tag: string): void;
}

const names = new Map<string, string>();
export const nameOf = (node: string) => names.get(node) ?? node.slice(0, 6);

const FRESH: Omit<GroupState, keyof GroupJson> = { floor: "idle", talkerName: null, talkerNode: null, handover: "stable", link: null, profile: "std", suspended: false, unread: 0 };

export function setGroups(gs: GroupJson[]) {
  const prev = new Map(useStore.getState().groups.map((g) => [g.id, g]));
  const groups: GroupState[] = gs.map((g) => {
    for (const m of g.members) names.set(m.node, m.name);
    return { ...(prev.get(g.id) ?? FRESH), ...g };
  });
  useStore.getState().set({ groups, activeGroup: gs.find((g) => g.isActive)?.id ?? null });
}

function updActive(f: (g: GroupState) => GroupState) {
  const a = useStore.getState().activeGroup;
  if (a) useStore.getState().updateGroup(a, f);
}

export function applyUi(e: UiEvent, h: ReducerHooks) {
  const st = useStore.getState();
  const me = st.nodeId;
  switch (e.type) {
    case "peerDiscovered":
      names.set(e.node, e.name);
      st.set((s) => ({ peers: { ...s.peers, [e.node]: { ...(s.peers[e.node] ?? { bars: 0, hops: 0 }), node: e.node, name: e.name, hue: e.hue, link: e.link, inGroup: e.in_group } } }));
      break;
    case "peerLost":
      st.set((s) => { const p = { ...s.peers }; delete p[e.node]; return { peers: p }; });
      break;
    case "peerLink":
      st.set((s) => (s.peers[e.node] ? { peers: { ...s.peers, [e.node]: { ...s.peers[e.node]!, link: e.link, bars: e.bars, hops: e.hops } } } : {}));
      break;
    case "floorGranted":
      updActive((g) => ({ ...g, floor: "talking", talkerName: null, talkerNode: me }));
      h.cue("granted");
      break;
    case "floorDenied":
      updActive((g) => ({ ...g, floor: "busy" }));
      h.cue("denied");
      break;
    case "floorTaken":
      names.set(e.holder, e.name);
      st.updateGroup(e.group, (g) => ({ ...g, floor: e.holder === me ? "talking" : "busy", talkerName: e.name, talkerNode: e.holder }));
      if (e.holder !== me) h.cue("incoming");
      break;
    case "floorIdle": {
      const wasMine = st.groups.find((g) => g.id === e.group)?.talkerNode === me;
      st.updateGroup(e.group, (g) => ({ ...g, floor: "idle", talkerName: null, talkerNode: null }));
      if (wasMine) h.cue("released");
      break;
    }
    case "talkWarning": st.set({ toast: "10 seconds left" }); h.cue("warning"); break;
    case "talkTimeout": st.set({ toast: "Talk time limit reached" }); break;
    case "inviteOffered":
      st.set((s) => ({ invites: [...s.invites.filter((i) => i.group !== e.group), { group: e.group, name: e.name, host: e.host, hostName: e.host_name, members: e.members }] }));
      h.cue("incoming");
      h.notify?.(`${e.host_name} invites you`, `Join “${e.name}” · ${e.members} members`, `invite-${e.group}`);
      break;
    case "joined": h.refreshGroups(); h.syncRooms(); st.set({ toast: `Joined ${e.name}` }); break;
    case "joinFailed": st.set({ toast: `Could not join: ${e.reason}` }); break;
    case "memberJoined": names.set(e.node, e.name); h.refreshGroups(); break;
    case "memberLeft": h.refreshGroups(); break;
    case "groupDissolved": st.set({ toast: `“${e.name}” was deleted by its creator` }); h.refreshGroups(); h.syncRooms(); break;
    case "message": {
      const m: Msg = { id: e.msg_uuid, group: e.group, from: e.from, fromName: nameOf(e.from), sentMs: e.sent_ms, body: e.body, mine: e.from === me, acked: false };
      st.set((s) => ({ messages: [...s.messages.filter((x) => x.id !== m.id), m].slice(-500) }));
      if (!m.mine && st.activeGroup !== e.group) st.updateGroup(e.group, (g) => ({ ...g, unread: g.unread + 1 }));
      if (!m.mine) {
        if (e.body.kind === "sos") {
          const b = e.body;
          const groupName = st.groups.find((g) => g.id === e.group)?.name ?? "";
          st.set((s) => ({ sos: b.cancelled ? (s.sos?.group === e.group ? null : s.sos) : { group: e.group, groupName, fromName: m.fromName, latE7: b.lat_e7, lonE7: b.lon_e7, note: b.note } }));
          h.cue("sos");
          h.notify?.(b.cancelled ? `${m.fromName} is safe` : `SOS from ${m.fromName}`, b.note || "Emergency", `sos-${e.group}`);
        }
        else if (e.body.kind === "text") h.notify?.(m.fromName, e.body.text, `msg-${e.group}`);
      }
      break;
    }
    case "messageAcked": st.set((s) => ({ messages: s.messages.map((m) => (m.id === e.msg_uuid ? { ...m, acked: true } : m)) })); break;
    case "handover": st.updateGroup(e.group, (g) => ({ ...g, handover: e.state, link: e.link, profile: e.profile })); break;
    case "suspended": st.updateGroup(e.group, (g) => ({ ...g, suspended: true })); break;
    case "resumed": st.updateGroup(e.group, (g) => ({ ...g, suspended: false })); break;
    case "modeChanged": st.updateGroup(e.group, (g) => ({ ...g, fullDuplex: e.full_duplex })); break;
    case "level": if (e.talker !== me) st.set({ levelDbfs: e.dbfs }); break;
    case "error": st.set({ toast: e.message }); break;
  }
}
