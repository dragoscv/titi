// Brand pack build: tokens -> outputs -> icons -> motion -> marketing.
//   node brand/scripts/build.mjs           regenerate everything
//   node brand/scripts/build.mjs --check   CI drift gate (exit 1 when any generated file differs)
import { readFileSync } from "node:fs";
import { writeTokens, writeOutputs } from "./tokens.mjs";
import { writeIcons } from "./icons.mjs";
import { writeMotion } from "./motion.mjs";
import { writeMarketing } from "./marketing.mjs";
import { writeDemo } from "./demo.mjs";
import { finish, out } from "./io.mjs";

process.chdir(new URL("../..", import.meta.url).pathname.replace(/^\/(\w:)/, "$1"));
writeTokens();
writeOutputs();
writeIcons();
writeMotion();
writeDemo();
await writeMarketing();
// fonts: one copy of each verified file per consumer (glyph gate ran on brand/fonts/*)
for (const f of ["Manrope-Variable.woff2", "GeistMono-Variable.woff2", "LICENSE-Manrope.txt", "LICENSE-GeistMono.txt"]) out(`packages/app-ui/src/fonts/${f}`, readFileSync(`brand/fonts/${f}`));
for (const f of ["Manrope-Medium.ttf", "Manrope-Bold.ttf", "Manrope-ExtraBold.ttf", "GeistMono-Medium.ttf"]) out(`ios/Titi/Resources/Fonts/${f}`, readFileSync(`brand/fonts/${f}`));
finish("brand");
