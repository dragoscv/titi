// Thumbnail-toolbar icons for the Windows taskbar preview (lucide paths, white stroke).
// Run: pnpm dlx -p @resvg/resvg-js node scripts/gen-thumbbar-icons.mjs
import { writeFileSync, mkdirSync } from "node:fs";
import { Resvg } from "@resvg/resvg-js";

const icons = {
  // lucide "radio" = start talking, "square" = stop, "mic-off" = mute, "mic" = unmute
  talk: '<path d="M16.247 7.761a6 6 0 0 1 0 8.478"/><path d="M19.075 4.933a10 10 0 0 1 0 14.134"/><path d="M4.925 19.067a10 10 0 0 1 0-14.134"/><path d="M7.753 16.239a6 6 0 0 1 0-8.478"/><circle cx="12" cy="12" r="2"/>',
  stop: '<rect x="5" y="5" width="14" height="14" rx="3" fill="#ffffff"/>',
  unmute: '<rect x="9" y="2" width="6" height="13" rx="3"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><path d="M12 19v3"/>',
  mute: '<path d="M12 19v3"/><path d="M15 9.34V5a3 3 0 0 0-5.68-1.33"/><path d="M16.95 16.95A7 7 0 0 1 5 12v-2"/><path d="M18.89 13.23A7 7 0 0 0 19 12v-2"/><path d="m2 2 20 20"/><path d="M9 9v3a3 3 0 0 0 5.12 2.12"/>',
};
const dir = new URL("../apps/desktop/src-tauri/icons/thumbbar/", import.meta.url);
mkdirSync(dir, { recursive: true });
for (const [name, body] of Object.entries(icons)) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`;
  writeFileSync(new URL(`${name}.png`, dir), new Resvg(svg, { fitTo: { mode: "width", value: 32 } }).render().asPng());
}
console.log("thumbbar icons written");
