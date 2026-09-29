#!/usr/bin/env node
// commit-msg hook: Conventional Commits (type(scope)?: subject), subject <= 100 chars.
import { readFileSync } from "node:fs";

const TYPES = ["feat", "fix", "perf", "refactor", "docs", "test", "build", "ci", "chore", "revert", "style", "security"];
const file = process.argv[2];
if (!file) process.exit(0);
const first = readFileSync(file, "utf8").split(/\r?\n/).find((l) => l.trim() && !l.startsWith("#")) ?? "";
if (/^(Merge|Revert|fixup!|squash!)/.test(first)) process.exit(0);
const re = new RegExp(`^(${TYPES.join("|")})(\\([a-z0-9,./-]+\\))?!?: \\S.{0,98}$`);
if (!re.test(first)) {
  console.error(`commit-msg: "${first}"\n  expected: <type>(<scope>)?: <subject>   types: ${TYPES.join(", ")}   (max 100 chars)`);
  process.exit(1);
}
