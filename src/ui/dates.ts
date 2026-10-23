const DAY = 86_400_000;

/**
 * "today", "yesterday", "5 days ago", "3 weeks ago", "4 months ago" for a `YYYY-MM-DD` day.
 * Unparseable input comes back as is.
 */
export function relativeDay(day: string, now: Date = new Date()): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
  if (!match) return day;
  const then = Date.UTC(
    Number(match[1]),
    Number(match[2]) - 1,
    Number(match[3]),
  );
  const today = Date.UTC(now.getFullYear(), now.getMonth(), now.getDate());
  const days = Math.round((today - then) / DAY);
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 14) return `${days} days ago`;
  const weeks = Math.floor(days / 7);
  if (days < 60) return `${weeks} weeks ago`;
  const months = Math.floor(days / 30);
  if (days < 365) return `${months} months ago`;
  const years = Math.floor(days / 365);
  return years === 1 ? "1 year ago" : `${years} years ago`;
}
