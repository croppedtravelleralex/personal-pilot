export function formatCount(value: number): string {
  return new Intl.NumberFormat("en-US").format(value);
}

function parseTimestamp(value: string): number | null {
  const numericValue = Number(value);
  if (Number.isFinite(numericValue) && numericValue > 0) {
    return numericValue;
  }

  const parsedMs = Date.parse(value);
  if (!Number.isNaN(parsedMs)) {
    return Math.floor(parsedMs / 1000);
  }

  return null;
}

export function formatRelativeTimestamp(value: string | null): string {
  if (!value) {
    return "Not recorded";
  }

  const seconds = parseTimestamp(value);
  if (!seconds) {
    return value;
  }

  const delta = Math.max(0, Math.floor(Date.now() / 1000) - seconds);
  if (delta < 60) {
    return `${delta}s ago`;
  }
  if (delta < 3600) {
    return `${Math.floor(delta / 60)}m ago`;
  }
  if (delta < 86400) {
    return `${Math.floor(delta / 3600)}h ago`;
  }
  return `${Math.floor(delta / 86400)}d ago`;
}

export function formatStatusLabel(status: string): string {
  return status.replaceAll("_", " ").toUpperCase();
}

export function formatRunnerLabel(value: string): string {
  return value
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}
