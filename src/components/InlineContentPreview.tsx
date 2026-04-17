import { useEffect, useState } from "react";

const DEFAULT_COLLAPSE_AT = 220;
const DEFAULT_INLINE_LIMIT = 12000;

function joinClassNames(...values: Array<string | false | null | undefined>) {
  return values.filter(Boolean).join(" ");
}

function normalizeInlineContent(value: string | null | undefined): string {
  return typeof value === "string" ? value.replace(/\r\n/g, "\n") : "";
}

export function truncateInlineContent(
  value: string | null | undefined,
  maxChars = DEFAULT_COLLAPSE_AT,
): string {
  const normalized = normalizeInlineContent(value);
  if (normalized.length <= maxChars) {
    return normalized;
  }

  return `${normalized.slice(0, Math.max(0, maxChars - 1)).trimEnd()}...`;
}

interface InlineContentPreviewProps {
  value: string | null | undefined;
  empty?: string;
  collapseAt?: number;
  inlineLimit?: number;
  expandable?: boolean;
  copyable?: boolean;
  mono?: boolean;
  muted?: boolean;
  className?: string;
  bodyClassName?: string;
}

export function InlineContentPreview({
  value,
  empty = "None",
  collapseAt = DEFAULT_COLLAPSE_AT,
  inlineLimit = DEFAULT_INLINE_LIMIT,
  expandable = true,
  copyable = true,
  mono = false,
  muted = false,
  className,
  bodyClassName,
}: InlineContentPreviewProps) {
  const normalized = normalizeInlineContent(value);
  const hasContent = normalized.trim().length > 0;
  const previewLimit = Math.max(24, Math.min(collapseAt, inlineLimit));
  const collapsedValue = truncateInlineContent(normalized, previewLimit);
  const limitedExpandedValue =
    normalized.length > inlineLimit
      ? `${normalized.slice(0, Math.max(0, inlineLimit - 1)).trimEnd()}...`
      : normalized;
  const canExpand = hasContent && expandable && normalized.length > previewLimit;
  const shouldShortenInline = hasContent && normalized.length > previewLimit;
  const needsExpandedInlineCap = hasContent && normalized.length > inlineLimit;
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    setExpanded(false);
    setCopied(false);
  }, [normalized, previewLimit, inlineLimit, expandable, copyable]);

  useEffect(() => {
    if (!copied) {
      return undefined;
    }

    const timer = window.setTimeout(() => setCopied(false), 1600);
    return () => window.clearTimeout(timer);
  }, [copied]);

  const displayValue = !hasContent
    ? empty
    : canExpand
      ? expanded
        ? limitedExpandedValue
        : collapsedValue
      : shouldShortenInline
        ? collapsedValue
        : normalized;
  const showActions = hasContent && (canExpand || (copyable && (canExpand || needsExpandedInlineCap)));

  async function handleCopy() {
    if (!hasContent || !copyable || typeof navigator === "undefined" || !navigator.clipboard) {
      return;
    }

    try {
      await navigator.clipboard.writeText(normalized);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  }

  return (
    <div className={joinClassNames("inline-content-preview", className)}>
      <div
        className={joinClassNames(
          "inline-content-preview__body",
          muted && "inline-content-preview__body--muted",
          mono && "inline-content-preview__body--mono",
          bodyClassName,
        )}
      >
        {displayValue}
      </div>
      {showActions ? (
        <div className="inline-actions inline-content-preview__actions">
          {canExpand ? (
            <button
              className="button button--secondary"
              type="button"
              onClick={() => setExpanded((current) => !current)}
            >
              {expanded ? "Collapse" : "Expand"}
            </button>
          ) : null}
          {copyable && (canExpand || needsExpandedInlineCap) ? (
            <button
              className="button button--secondary"
              type="button"
              onClick={() => void handleCopy()}
            >
              {copied ? "Copied" : "Copy full text"}
            </button>
          ) : null}
        </div>
      ) : null}
      {hasContent && expanded && needsExpandedInlineCap ? (
        <div className="inline-content-preview__note">
          Inline render is capped at {inlineLimit.toLocaleString()} chars to keep the desktop view stable.
          Use copy to capture the full value.
        </div>
      ) : null}
    </div>
  );
}
