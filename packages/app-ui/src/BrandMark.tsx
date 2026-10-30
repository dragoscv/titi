import { MARK_PARTS, MARK_SLASH, MARK_VIEWBOX, type MarkState } from "./brand-mark";
// Animation CSS ships via theme.css (brand-mark.css), which every host imports.

/**
 * The Titi "Chirp" mark (brand pack v1). Geometry and motion are generated from brand/ —
 * never hand-edit the paths. `state` picks a logo animation; reduced motion shows the final frame.
 */
export function BrandMark({ size = 48, state = "idle", label = "Titi", className }: { size?: number; state?: MarkState; label?: string; className?: string }) {
  return (
    <svg className={`titi-mark${className ? ` ${className}` : ""}`} viewBox={MARK_VIEWBOX} width={size} height={size} data-mark-state={state}>
      <title>{label}</title>
      <g data-part="mark">
        {MARK_PARTS.map((p) =>
          "stroke" in p ? (
            <path key={p.name} data-part={p.name} d={p.d} stroke={p.stroke} strokeWidth={p.width} strokeLinecap="round" fill="none" pathLength={1} />
          ) : (
            <path key={p.name} data-part={p.name} d={p.d} fill={p.fill} />
          ),
        )}
        {state === "muted" && <path data-part="slash" d={MARK_SLASH.d} stroke="var(--color-danger-solid)" strokeWidth={MARK_SLASH.width} strokeLinecap="round" />}
      </g>
    </svg>
  );
}
