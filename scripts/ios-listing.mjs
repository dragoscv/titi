// Applies docs/store/ios/listing.<locale>.json to App Store Connect (app 6829145747, shared team key).
//   node scripts/ios-listing.mjs            apply version + app info metadata, categories, review contact
//   node scripts/ios-listing.mjs --dry      print what would change
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const { asc } = await import(pathToFileURL(join(homedir(), ".copilot", "bin", "apple-asc.mjs")).href);
const APP = "6829145747";
const VERSION = "0.1.0";
const LOCALES = ["en-US", "ro"];
const DRY = process.argv.includes("--dry");

async function call(method, path, body) {
  if (DRY && method !== "GET") { console.log("DRY", method, path, JSON.stringify(body?.data?.attributes ?? {}).slice(0, 200)); return { data: { id: "dry" } }; }
  const r = await asc(method, path, body);
  if (r.status >= 300) throw new Error(`${method} ${path} -> ${r.status} ${JSON.stringify(r.json?.errors ?? []).slice(0, 500)}`);
  return r.json;
}
const listing = Object.fromEntries(LOCALES.map((l) => [l, JSON.parse(readFileSync(join("docs", "store", "ios", `listing.${l}.json`), "utf8"))]));

// ---- version: rename the placeholder 1.0 to the build's MARKETING_VERSION, automatic release after approval
const versions = (await call("GET", `/v1/apps/${APP}/appStoreVersions?filter[platform]=IOS&limit=5`)).data;
const ver = versions.find((v) => ["PREPARE_FOR_SUBMISSION", "DEVELOPER_REJECTED", "REJECTED", "METADATA_REJECTED"].includes(v.attributes.appStoreState));
if (!ver) throw new Error("no editable App Store version");
await call("PATCH", `/v1/appStoreVersions/${ver.id}`, { data: { type: "appStoreVersions", id: ver.id, attributes: { versionString: VERSION, releaseType: "AFTER_APPROVAL", copyright: `${new Date().getFullYear()} Dragos-Catalin Vladulescu` } } });
console.log(`version ${ver.id}: ${ver.attributes.versionString} -> ${VERSION}, automatic release`);

// ---- version localizations (description, keywords, promo, what's new, URLs)
const vlocs = (await call("GET", `/v1/appStoreVersions/${ver.id}/appStoreVersionLocalizations`)).data;
for (const locale of LOCALES) {
  const l = listing[locale];
  const attrs = { description: l.description, keywords: l.keywords, promotionalText: l.promotionalText, supportUrl: l.supportUrl, marketingUrl: l.marketingUrl };
  // whatsNew is rejected on a first version
  const ex = vlocs.find((x) => x.attributes.locale === locale);
  if (ex) await call("PATCH", `/v1/appStoreVersionLocalizations/${ex.id}`, { data: { type: "appStoreVersionLocalizations", id: ex.id, attributes: attrs } });
  else await call("POST", "/v1/appStoreVersionLocalizations", { data: { type: "appStoreVersionLocalizations", attributes: { locale, ...attrs }, relationships: { appStoreVersion: { data: { type: "appStoreVersions", id: ver.id } } } } });
  console.log(`version localization ${locale}: ok`);
}

// ---- app info: name, subtitle, privacy policy per locale; categories
const info = (await call("GET", `/v1/apps/${APP}/appInfos`)).data.find((i) => i.attributes.state !== "READY_FOR_DISTRIBUTION") ?? (await call("GET", `/v1/apps/${APP}/appInfos`)).data[0];
const ilocs = (await call("GET", `/v1/appInfos/${info.id}/appInfoLocalizations`)).data;
for (const locale of LOCALES) {
  const l = listing[locale];
  const attrs = { name: l.name, subtitle: l.subtitle, privacyPolicyUrl: l.privacyPolicyUrl };
  const ex = ilocs.find((x) => x.attributes.locale === locale);
  if (ex) await call("PATCH", `/v1/appInfoLocalizations/${ex.id}`, { data: { type: "appInfoLocalizations", id: ex.id, attributes: attrs } });
  else await call("POST", "/v1/appInfoLocalizations", { data: { type: "appInfoLocalizations", attributes: { locale, ...attrs }, relationships: { appInfo: { data: { type: "appInfos", id: info.id } } } } });
  console.log(`app info localization ${locale}: ok`);
}
await call("PATCH", `/v1/appInfos/${info.id}`, {
  data: {
    type: "appInfos", id: info.id,
    relationships: {
      primaryCategory: { data: { type: "appCategories", id: "SOCIAL_NETWORKING" } },
      secondaryCategory: { data: { type: "appCategories", id: "UTILITIES" } },
    },
  },
});
console.log("categories: Social Networking + Utilities");

// ---- App Review contact + notes (notes block from review-notes.md, placeholders filled from env/args)
const notesMd = readFileSync(join("docs", "store", "ios", "review-notes.md"), "utf8");
const m = notesMd.match(/<!-- NOTES START -->([\s\S]*?)<!-- NOTES END -->/);
if (!m) throw new Error("review-notes.md has no NOTES block");
const demoLink = readFileSync(join("docs", "store", "ios", "demo-link.txt"), "utf8").trim();
const video = process.env.TITI_DEMO_VIDEO_URL ?? "";
let notes = m[1].trim().replaceAll("{{DEMO_LINK}}", demoLink);
notes = video ? notes.replaceAll("{{DEMO_VIDEO_URL}}", video) : notes.replace(/^.*\{\{DEMO_VIDEO_URL\}\}.*$\n?/gm, "");
if (notes.length > 4000) throw new Error(`review notes ${notes.length} > 4000`);
// Same owner as codai: reuse its App Review contact phone unless TITI_REVIEW_PHONE is set (never printed).
async function ownerPhone() {
  if (process.env.TITI_REVIEW_PHONE) return process.env.TITI_REVIEW_PHONE;
  const v = await asc("GET", "/v1/apps/6827785088/appStoreVersions?filter[platform]=IOS&limit=1");
  const id = v.json?.data?.[0]?.id;
  const d = id ? await asc("GET", `/v1/appStoreVersions/${id}/appStoreReviewDetail`) : null;
  return d?.json?.data?.attributes?.contactPhone ?? "";
}
const reviewAttrs = {
  contactFirstName: "Dragos-Catalin", contactLastName: "Vladulescu", contactEmail: "support@codai.ro",
  contactPhone: await ownerPhone(), demoAccountRequired: false, notes,
};
if (!reviewAttrs.contactPhone) throw new Error("set TITI_REVIEW_PHONE (App Review requires a contact phone)");
const rd = await asc("GET", `/v1/appStoreVersions/${ver.id}/appStoreReviewDetail`);
if (rd.status === 200 && rd.json?.data?.id) {
  await call("PATCH", `/v1/appStoreReviewDetails/${rd.json.data.id}`, { data: { type: "appStoreReviewDetails", id: rd.json.data.id, attributes: reviewAttrs } });
} else {
  await call("POST", "/v1/appStoreReviewDetails", { data: { type: "appStoreReviewDetails", attributes: reviewAttrs, relationships: { appStoreVersion: { data: { type: "appStoreVersions", id: ver.id } } } } });
}
console.log(`review details: notes ${notes.length} chars${video ? "" : " (no video URL yet)"}`);
