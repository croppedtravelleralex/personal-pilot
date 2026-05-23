import type { DesktopValidationReport } from "../../types/desktop";

export type ValidationEvidenceLayer = "declared" | "applied" | "observed";

export type ValidationEvidenceStatus = "ready" | "partial" | "missing";

export interface ValidationEvidenceItem {
  id: string;
  label: string;
  layer: ValidationEvidenceLayer;
  status: ValidationEvidenceStatus;
  signalCount: number;
  summary: string;
}

export interface ValidationCategory {
  id: string;
  label: string;
  description: string;
  items: ValidationEvidenceItem[];
}

export interface ValidationBoardSnapshot {
  categories: ValidationCategory[];
  generatedAt: string;
}

export interface ValidationBoardSummary {
  categoryCount: number;
  declaredCount: number;
  appliedCount: number;
  observedCount: number;
  missingCount: number;
  readyCount: number;
}

const CATEGORY_DEFINITIONS: Array<Omit<ValidationCategory, "items">> = [
  {
    id: "detector",
    label: "Detector",
    description: "Bot and platform detection outcomes.",
  },
  {
    id: "leak",
    label: "Leak",
    description: "Identity, storage, and cross-profile leak checks.",
  },
  {
    id: "dns",
    label: "DNS",
    description: "Resolver path, region, and proxy coherence.",
  },
  {
    id: "webrtc",
    label: "WebRTC",
    description: "Local IP, public IP, and media device exposure.",
  },
  {
    id: "canvas",
    label: "Canvas",
    description: "Canvas and WebGL identity materialization.",
  },
  {
    id: "audio",
    label: "Audio",
    description: "AudioContext and device signal stability.",
  },
  {
    id: "worker",
    label: "Worker",
    description: "Worker, service worker, and off-main-thread state.",
  },
  {
    id: "transport",
    label: "Transport",
    description: "TLS, HTTP, proxy, and request-path evidence.",
  },
];

const READY_APPLIED = new Set(["dns", "transport"]);
const PARTIAL_APPLIED = new Set(["detector", "leak", "canvas", "worker"]);
const PARTIAL_OBSERVED = new Set(["dns", "transport"]);

function buildEvidenceItem(
  category: Omit<ValidationCategory, "items">,
  layer: ValidationEvidenceLayer,
): ValidationEvidenceItem {
  if (layer === "declared") {
    return {
      id: `${category.id}-declared`,
      label: "Declared",
      layer,
      status: "ready",
      signalCount: 1,
      summary: "Control surface is documented in the target evidence board.",
    };
  }

  if (layer === "applied") {
    const ready = READY_APPLIED.has(category.id);
    const partial = PARTIAL_APPLIED.has(category.id);
    return {
      id: `${category.id}-applied`,
      label: "Applied",
      layer,
      status: ready ? "ready" : partial ? "partial" : "missing",
      signalCount: ready ? 2 : partial ? 1 : 0,
      summary: ready
        ? "Runtime or provider path has a concrete local contract."
        : partial
          ? "Some code boundary exists, but the runtime path is incomplete."
          : "No applied runtime evidence is registered yet.",
    };
  }

  const observed = PARTIAL_OBSERVED.has(category.id);
  return {
    id: `${category.id}-observed`,
    label: "Observed",
    layer,
    status: observed ? "partial" : "missing",
    signalCount: observed ? 1 : 0,
    summary: observed
      ? "Operational signals exist, but no repeatable board report is available yet."
      : "No repeatable observed evidence report is available yet.",
  };
}

function observedStatusFromReport(
  categoryId: string,
  report?: DesktopValidationReport | null,
): ValidationEvidenceStatus | null {
  const signals = report?.signals.filter((signal) => signal.category === categoryId) ?? [];
  if (signals.length === 0) return null;
  if (signals.every((signal) => signal.status === "succeeded")) return "ready";
  if (signals.some((signal) => signal.status !== "failed")) return "partial";
  return "missing";
}

function observedSummaryFromReport(
  categoryId: string,
  report?: DesktopValidationReport | null,
): string | null {
  const signals = report?.signals.filter((signal) => signal.category === categoryId) ?? [];
  if (signals.length === 0) return null;
  return signals.map((signal) => signal.summary).join(" ");
}

function applyObservedReport(
  snapshot: ValidationBoardSnapshot,
  report?: DesktopValidationReport | null,
): ValidationBoardSnapshot {
  if (!report) return snapshot;
  return {
    ...snapshot,
    categories: snapshot.categories.map((category) => ({
      ...category,
      items: category.items.map((item) => {
        if (item.layer !== "observed") return item;
        const status = observedStatusFromReport(category.id, report);
        const summary = observedSummaryFromReport(category.id, report);
        if (!status || !summary) return item;
        const signalCount = report.signals.filter(
          (signal) => signal.category === category.id,
        ).length;
        return {
          ...item,
          status,
          signalCount,
          summary,
        };
      }),
    })),
  };
}

export function buildValidationBoardSnapshot(
  report?: DesktopValidationReport | null,
): ValidationBoardSnapshot {
  const snapshot = {
    generatedAt: report?.generatedAt ?? new Date().toISOString(),
    categories: CATEGORY_DEFINITIONS.map((category) => ({
      ...category,
      items: [
        buildEvidenceItem(category, "declared"),
        buildEvidenceItem(category, "applied"),
        buildEvidenceItem(category, "observed"),
      ],
    })),
  };

  return applyObservedReport(snapshot, report);
}

export function summarizeValidationBoard(
  snapshot: ValidationBoardSnapshot,
): ValidationBoardSummary {
  const items = snapshot.categories.flatMap((category) => category.items);
  return {
    categoryCount: snapshot.categories.length,
    declaredCount: items.filter((item) => item.layer === "declared").length,
    appliedCount: items.filter(
      (item) => item.layer === "applied" && item.status !== "missing",
    ).length,
    observedCount: items.filter(
      (item) => item.layer === "observed" && item.status !== "missing",
    ).length,
    missingCount: items.filter((item) => item.status === "missing").length,
    readyCount: items.filter((item) => item.status === "ready").length,
  };
}
