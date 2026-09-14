/* @ts-self-types="./titi_wasm.d.ts" */
import * as wasm from "./titi_wasm_bg.wasm";
import { __wbg_set_wasm } from "./titi_wasm_bg.js";

__wbg_set_wasm(wasm);

export {
    WasmEngine, group_hash, invite_wordlist, parse_invite_code, rendezvous_for_code
} from "./titi_wasm_bg.js";
