/** Columns for n tiles: 21:9 fits 4 across, 16:9 fits 3; rows are balanced (7 on 21:9 → 4+3, not 4+… ). */
export function dashboardColumns(n: number, aspect: number): number {
  if (n <= 0) return 1;
  const max = aspect >= 2.1 ? 4 : aspect >= 1.6 ? 3 : 2;
  const rows = Math.ceil(n / max);
  return Math.min(max, Math.ceil(n / rows));
}
