#!/usr/bin/env node
// Exit 0 ("skip") when none of the given paths changed between upstream and HEAD; exit 1 otherwise.
// Used by lefthook to skip the slow native lane on JS-only pushes. No upstream → never skip.
import { execFileSync } from "node:child_process";

const paths = process.argv.slice(2);
try {
  const base = execFileSync("git", ["merge-base", "HEAD", "@{upstream}"], { encoding: "utf8" }).trim();
  const out = execFileSync("git", ["diff", "--name-only", base, "HEAD", "--", ...paths], { encoding: "utf8" }).trim();
  process.exit(out ? 1 : 0);
} catch {
  process.exit(1);
}
