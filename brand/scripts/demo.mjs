// Live logo-motion demo (Gate E): brand/demo/motion.html, uses the generated CSS + geometry only.
import { out } from "./io.mjs";
import { logoCss, VIEWBOX, markBody } from "./motion.mjs";
import { C } from "./mark.mjs";
import { MOTION } from "./motion-spec.mjs";

export function writeDemo() {
  const parts = markBody();
  const states = Object.entries(MOTION.logo);
  const cards = states
    .map(([n, t]) => `<figure><button type="button" aria-label="Replay ${n}" data-state="${n}"><svg class="titi-mark" viewBox="${VIEWBOX.join(" ")}" data-mark-state="${n}" role="img" aria-label="Titi ${n}">${parts}</svg></button><figcaption><b>${n}</b> · ${t.$value.duration} ms${t.$value.loop ? " · loop" : ""}<br>${t.$description}</figcaption></figure>`)
    .join("\n");
  const html = `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Titi logo motion</title>
<style>${logoCss()}
body{margin:0;background:${C.graphite};color:${C.ink};font:15px/1.45 Manrope,system-ui,sans-serif;padding:32px}
h1{font-size:28px;margin:0 0 4px}p{color:${C.muted};margin:0 0 24px}
main{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:20px}
figure{margin:0;background:#16191E;border:1px solid #2A2F38;border-radius:22px;padding:18px}
button{all:unset;cursor:pointer;display:block;border-radius:16px}button:focus-visible{outline:3px solid ${C.amber};outline-offset:4px}
svg{width:100%;height:160px}figcaption{color:${C.muted};font-size:13px;margin-top:10px}figcaption b{color:${C.ink}}
label{display:inline-flex;gap:8px;align-items:center;color:${C.muted};margin-bottom:20px}
.rm .titi-mark [data-part]{animation:none!important}</style>
<h1>Titi · logo motion</h1><p>Generated from brand/tokens/motion.tokens.json → logo.*. Click a card to replay. Static mark = final frame of intro.</p>
<label><input type="checkbox" id="rm"> Simulate prefers-reduced-motion</label>
<main>${cards}</main>
<script>
document.querySelectorAll("button[data-state]").forEach(b=>b.addEventListener("click",()=>{const s=b.querySelector("svg");const v=s.dataset.markState;s.dataset.markState="";void s.getBoundingClientRect();s.dataset.markState=v;}));
document.querySelectorAll("button[data-state=hover],button[data-state=press]").forEach(b=>{const s=b.querySelector("svg");const st=b.dataset.state;s.dataset.markState="idle";const on=()=>s.dataset.markState=st,off=()=>s.dataset.markState="idle";if(st==="hover"){b.addEventListener("pointerenter",on);b.addEventListener("pointerleave",off);b.addEventListener("focus",on);b.addEventListener("blur",off);}else{b.addEventListener("pointerdown",on);b.addEventListener("pointerup",off);b.addEventListener("pointerleave",off);}});
document.getElementById("rm").addEventListener("change",e=>document.body.classList.toggle("rm",e.target.checked));
</script></html>`;
  out("brand/demo/motion.html", html);
}
