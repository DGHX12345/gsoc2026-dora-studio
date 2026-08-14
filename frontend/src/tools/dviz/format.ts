// Pure formatting helpers for the dviz path control panel (M12 D4).
// Framework-free and side-effect-free so the tool test runner can cover them.

/** Format a path length in meters with 2 decimals, e.g. 0.457 → "0.46 m". */
export function formatLength(meters: number): string {
  return `${meters.toFixed(2)} m`;
}

/** Format an xyz position with 2 decimals, e.g. "(0.12, -0.05, 0.02)".
 * toFixed can render "-0.00" for tiny negative values; normalize so negative
 * zero displays as "0.00". */
export function formatPosition(x: number, y: number, z: number): string {
  const fmt = (v: number) => {
    const s = v.toFixed(2);
    return s === '-0.00' ? '0.00' : s;
  };
  return `(${fmt(x)}, ${fmt(y)}, ${fmt(z)})`;
}
