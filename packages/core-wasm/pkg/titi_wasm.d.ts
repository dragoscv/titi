/* tslint:disable */
/* eslint-disable */

export class WasmEngine {
    free(): void;
    [Symbol.dispose](): void;
    accept_invite(group: Uint8Array, host: Uint8Array, now_ms: number): string;
    create_group(name: string, now_ms: number): string;
    current_code(group: Uint8Array, now_ms: number): string | undefined;
    decline_invite(group: Uint8Array, host: Uint8Array, now_ms: number): string;
    deep_link(group: Uint8Array, now_ms: number, valid_ms: number): string | undefined;
    /**
     * Creator only: delete the group for every member.
     */
    dissolve_group(group: Uint8Array, now_ms: number): string;
    groups_json(): string;
    identity_seed(): Uint8Array;
    invite_peer(group: Uint8Array, node: Uint8Array, now_ms: number): string;
    is_talking(): boolean;
    join_by_code(code: string, now_ms: number): string;
    join_by_link(url: string, now_ms: number): string;
    leave_group(group: Uint8Array, now_ms: number): string;
    constructor(seed: Uint8Array, display_name: string, avatar_hue: number, rng_seed: bigint);
    node_id(): Uint8Array;
    on_frame(link: number, token: string, bytes: Uint8Array, now_ms: number): string;
    on_link_down(link: number, now_ms: number): string;
    on_link_stats(link: number, est_bps: number, rtt_ms: number, loss_pct: number, now_ms: number): string;
    on_link_up(link: number, _class: number, now_ms: number): string;
    /**
     * One encoded Opus packet from WebCodecs (20 ms, 48 kHz mono).
     */
    on_opus_in(packet: Uint8Array, now_ms: number): string;
    on_peer_lost(link: number, token: string, now_ms: number): string;
    on_peer_seen(link: number, token: string, now_ms: number): string;
    ptt_down(prio: number, now_ms: number): string;
    ptt_up(now_ms: number): string;
    /**
     * Concatenated 4-byte relay rendezvous hashes the host should join for `gid`.
     */
    rendezvous_for_group(group: Uint8Array, now_ms: number): Uint8Array;
    restore_groups(data: Uint8Array, now_ms: number): boolean;
    send_location(group: Uint8Array, lat: number, lon: number, accuracy_m: number, breadcrumb: boolean, now_ms: number): string;
    send_sos(group: Uint8Array, lat: number, lon: number, note: string, cancelled: boolean, now_ms: number): string;
    send_text(group: Uint8Array, text: string, now_ms: number): string;
    set_active_group(group: Uint8Array): void;
    set_display_name(name: string, hue: number): void;
    set_full_duplex(group: Uint8Array, on: boolean, now_ms: number): string;
    tick(now_ms: number): string;
}

export function group_hash(group: Uint8Array): Uint8Array;

export function invite_wordlist(): string[];

export function parse_invite_code(text: string): boolean;

/**
 * Concatenated 4-byte relay rendezvous hashes for a typed code (slots −1,0,+1).
 */
export function rendezvous_for_code(code: string, now_ms: number): Uint8Array;
