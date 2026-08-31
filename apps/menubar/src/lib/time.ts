const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/**
 * not Intl.RelativeTimeFormat, its "3 days ago" is wider than the column and
 * it has no way to say yesterday
 */
export function relativeTime(ms: number): string {
  const elapsed = Date.now() - ms;

  // a clock skew or a seed written a moment ahead
  if (elapsed < MINUTE) return "just now";
  if (elapsed < HOUR) return `${Math.floor(elapsed / MINUTE)}m ago`;
  if (elapsed < DAY) return `${Math.floor(elapsed / HOUR)}h ago`;

  const days = Math.floor(elapsed / DAY);
  if (days === 1) return "yesterday";
  if (days < 7) return `${days}d ago`;

  return new Date(ms).toLocaleDateString(undefined, { day: "numeric", month: "short" });
}
