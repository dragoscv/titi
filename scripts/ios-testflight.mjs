// TestFlight housekeeping for Titi (ro.titi.app) over the App Store Connect API, using the shared team key
// (~/.copilot/bin/apple-asc.mjs, secrets in dragos-devcloud; nothing printed).
//
//   node scripts/ios-testflight.mjs status
//   node scripts/ios-testflight.mjs tester --email you@example.com [--first A --last B]
//   node scripts/ios-testflight.mjs what-to-test [--build N]      texts: docs/store/ios/testflight/what-to-test.<locale>.txt
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const { asc } = await import(pathToFileURL(join(homedir(), ".copilot", "bin", "apple-asc.mjs")).href);
const BUNDLE_ID = "ro.titi.app";
const GROUP = "Titi internal";
const LOCALES = ["en-US", "ro"];
const arg = (k) => { const i = process.argv.indexOf(`--${k}`); return i > 0 ? process.argv[i + 1] : undefined; };

async function call(method, path, body) {
  const r = await asc(method, path, body);
  if (r.status >= 300) throw new Error(`${method} ${path} -> ${r.status} ${JSON.stringify(r.json?.errors?.[0] ?? {}).slice(0, 300)}`);
  return r.json;
}

const app = (await call("GET", `/v1/apps?filter[bundleId]=${BUNDLE_ID}`)).data[0];
if (!app) throw new Error(`no App Store Connect app for ${BUNDLE_ID}`);

async function group() {
  const gs = (await call("GET", `/v1/apps/${app.id}/betaGroups?limit=50`)).data;
  const g = gs.find((x) => x.attributes.name === GROUP);
  if (g) return g;
  return (await call("POST", "/v1/betaGroups", { data: { type: "betaGroups", attributes: { name: GROUP, isInternalGroup: true, hasAccessToAllBuilds: true }, relationships: { app: { data: { type: "apps", id: app.id } } } } })).data;
}

async function builds() {
  return (await call("GET", `/v1/builds?filter[app]=${app.id}&sort=-uploadedDate&limit=10&fields[builds]=version,processingState,expired,uploadedDate,minOsVersion`)).data;
}

const cmd = process.argv[2] ?? "status";
if (cmd === "status") {
  const g = await group();
  const testers = (await call("GET", `/v1/betaGroups/${g.id}/betaTesters?limit=50&fields[betaTesters]=email,state`)).data;
  console.log(JSON.stringify({
    app: app.id,
    group: g.id,
    testers: testers.map((t) => `${t.attributes.email} ${t.attributes.state}`),
    builds: (await builds()).map((b) => `${b.attributes.version} ${b.attributes.processingState}${b.attributes.expired ? " EXPIRED" : ""} ${b.attributes.uploadedDate}`),
  }, null, 2));
} else if (cmd === "tester") {
  const email = arg("email");
  if (!email) throw new Error("--email required");
  const g = await group();
  const found = (await call("GET", `/v1/betaTesters?filter[email]=${encodeURIComponent(email)}`)).data[0];
  let id = found?.id;
  if (!id) {
    id = (await call("POST", "/v1/betaTesters", { data: { type: "betaTesters", attributes: { email, firstName: arg("first") ?? "Dragos", lastName: arg("last") ?? "Vladulescu" }, relationships: { betaGroups: { data: [{ type: "betaGroups", id: g.id }] } } } })).data.id;
  } else {
    await call("POST", `/v1/betaGroups/${g.id}/relationships/betaTesters`, { data: [{ type: "betaTesters", id }] });
  }
  const r = await asc("POST", "/v1/betaTesterInvitations", { data: { type: "betaTesterInvitations", relationships: { app: { data: { type: "apps", id: app.id } }, betaTester: { data: { type: "betaTesters", id } } } } });
  console.log(`tester ${email}: in "${GROUP}", invitation ${r.status}`);
} else if (cmd === "what-to-test") {
  const want = arg("build");
  const b = (await builds()).find((x) => !want || x.attributes.version === want);
  if (!b) throw new Error("no build");
  const have = (await call("GET", `/v1/builds/${b.id}/betaBuildLocalizations`)).data;
  for (const locale of LOCALES) {
    const whatsNew = readFileSync(join("docs", "store", "ios", "testflight", `what-to-test.${locale}.txt`), "utf8").trim();
    if (whatsNew.length > 4000) throw new Error(`${locale}: ${whatsNew.length} > 4000`);
    const ex = have.find((x) => x.attributes.locale === locale);
    if (ex) await call("PATCH", `/v1/betaBuildLocalizations/${ex.id}`, { data: { type: "betaBuildLocalizations", id: ex.id, attributes: { whatsNew } } });
    else await call("POST", "/v1/betaBuildLocalizations", { data: { type: "betaBuildLocalizations", attributes: { locale, whatsNew }, relationships: { build: { data: { type: "builds", id: b.id } } } } });
    console.log(`what-to-test ${locale} set on build ${b.attributes.version}`);
  }
} else {
  throw new Error(`unknown command ${cmd}`);
}
