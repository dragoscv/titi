// Motion tokens (DTCG) incl. the parametric logo animation set. ONE source: CSS keyframes,
// the animated SVG and the Android AVD are all generated from MOTION.logo by motion.mjs.
// Tracks: part -> [[t(0..1), prop, value]]; props: scale | tx | ty | rot | opacity | draw (0..1 stroke trim).
// Pivots (grid units) come from the mark construction: body centre, tail tip, beak root.
export const PIVOTS = { mark: [22, 26], body: [22, 26], tail: [12, 38], beak: [32, 26], eye: [26.2, 22.4], arc: [22, 26], arc2: [22, 26] };

const ms = (value) => ({ $value: { value, unit: "ms" } });
const logo = (duration, ease, loop, tracks, meaning, pivots) => ({
  $type: "titi.logoAnimation",
  $description: meaning,
  $value: { duration, ease, loop, tracks, ...(pivots ? { pivots } : {}) },
});

export const MOTION = {
  $description: "Brand motion tokens. Every animation has a prefers-reduced-motion fallback (final frame / opacity only). The Talk spring is ADR-0007's stiffness 380 / damping 30.",
  duration: { $type: "duration", fast: ms(120), base: ms(200), slow: ms(320), brand: ms(900) },
  easing: {
    $type: "cubicBezier",
    enter: { $value: [0.05, 0.7, 0.1, 1], $description: "M3 emphasized decelerate" },
    exit: { $value: [0.3, 0, 0.8, 0.15], $description: "M3 emphasized accelerate" },
    move: { $value: [0.2, 0, 0, 1], $description: "M3 standard" },
    chirp: { $value: [0.34, 1.56, 0.64, 1], $description: "logo only: the 'chirp' overshoot (legacy --ease-spring)" },
  },
  spring: { $type: "titi.spring", talk: { $value: { stiffness: 380, damping: 30 } } },
  reduced: { $type: "duration", fade: { ...ms(150), $description: "max cross-fade under prefers-reduced-motion" } },
  logo: {
    intro: logo(900, "chirp", false, {
      body: [[0, "scale", 0], [0.33, "scale", 1]],
      tail: [[0, "scale", 0], [0.15, "scale", 0], [0.42, "scale", 1]],
      beak: [[0, "tx", -6], [0, "opacity", 0], [0.2, "opacity", 0], [0.22, "tx", -6], [0.28, "opacity", 1], [0.5, "tx", 0]],
      eye: [[0, "scale", 0], [0.4, "scale", 0], [0.55, "scale", 1]],
      arc: [[0, "draw", 0], [0, "opacity", 0], [0.5, "opacity", 0], [0.54, "opacity", 1], [0.5, "draw", 0], [1, "draw", 1]],
    }, "Bubble pops from its tail, beak slides out, the eye opens, the signal arc draws: 'a voice goes out'."),
    searching: logo(1600, "move", true, {
      arc: [[0, "draw", 0], [0, "opacity", 0], [0.06, "opacity", 1], [0.45, "draw", 1], [0.7, "opacity", 1], [1, "opacity", 0]],
      arc2: [[0, "opacity", 0], [0.25, "draw", 0], [0.31, "opacity", 1], [0.7, "draw", 1], [0.85, "opacity", 1], [1, "opacity", 0]],
    }, "Looking for peers / connecting: arcs keep reaching out. Pause after 5 s in view."),
    talking: logo(1000, "move", true, {
      arc: [[0, "opacity", 1], [0.5, "opacity", 0.45], [1, "opacity", 1]],
      arc2: [[0, "opacity", 0.2], [0.5, "opacity", 1], [1, "opacity", 0.2]],
      beak: [[0, "rot", 0], [0.25, "rot", -10], [0.5, "rot", 0], [0.75, "rot", -10], [1, "rot", 0]],
    }, "Transmitting or receiving: two arcs alternate, beak chatters."),
    success: logo(320, "chirp", false, {
      mark: [[0, "ty", 0], [0.4, "ty", -2.2], [1, "ty", 0]],
      arc: [[0, "scale", 1], [0.4, "scale", 1.08], [1, "scale", 1]],
    }, "Joined / delivered: one hop."),
    error: logo(180, "move", false, {
      mark: [[0, "tx", 0], [0.17, "tx", -1.6], [0.5, "tx", 1.6], [0.83, "tx", -1.2], [1, "tx", 0]],
      arc: [[0, "opacity", 1], [0.5, "opacity", 0.3], [1, "opacity", 1]],
    }, "Denied / no peers: short horizontal shake (never a colour flash alone)."),
    hover: logo(160, "move", false, {
      mark: [[0, "rot", 0], [1, "rot", -8]],
      arc: [[0, "scale", 1], [1, "scale", 1.06]],
    }, "Head tilt on hover/focus, the arc leans out (hold final frame)."),
    press: logo(120, "move", false, { mark: [[0, "scale", 1], [1, "scale", 0.92]] }, "Press: the whole bird compresses like the Talk button (hold until release)."),
    morph: logo(420, "enter", false, {
      tail: [[0, "scale", 1], [0.45, "scale", 0]],
      beak: [[0, "scale", 1], [0.4, "scale", 0]],
      eye: [[0, "scale", 1], [0.3, "scale", 0]],
      arc: [[0, "scale", 1], [0, "opacity", 1], [0.7, "scale", 1.3], [0.7, "opacity", 0]],
      body: [[0, "scale", 1], [0.25, "scale", 1], [1, "scale", 1.55]],
    }, "Splash -> UI: tail and beak tuck into the body, the eye closes, the signal ripples out and the body grows into the Talk button.", { tail: [22, 26], beak: [22, 26], eye: [26.2, 22.4], arc: [22, 26], body: [22, 26] }),
  },
};
