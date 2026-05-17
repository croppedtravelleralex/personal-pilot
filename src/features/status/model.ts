import type { DesktopStatusSnapshot, DesktopTaskItem } from "../../types/desktop";

type StatusTone = "neutral" | "success" | "warning" | "danger";
type StatusNoticeTone = "info" | "warning" | "error";

export const STATUS_AUTO_REFRESH_INTERVAL_MS = 30000;

export interface StatusMetric {
  label: string;
  value: string;
  detail: string;
  tone: StatusTone;
}

export interface StatusAttentionItem {
  id: string;
  tone: StatusNoticeTone;
  title: string;
  detail: string;
}

export interface StatusReviewBucket {
  id: string;
  label: string;
  value: string;
  detail: string;
  tone: StatusTone;
}

export interface StatusOverviewSummary {
  postureLabel: string;
  postureDetail: string;
  postureTone: StatusTone;
  successRateLabel: string;
  failureDebt: number;
  manualGateCount: number;
  abnormalTaskCount: number;
  browserTaskCount: number;
  queuePressureLabel: string;
  queuePressureDetail: string;
  operationsHeadline: string;
  operationsDetail: string;
  reviewTasks: DesktopTaskItem[];
  reviewBuckets: StatusReviewBucket[];
  metrics: StatusMetric[];
  attentionItems: StatusAttentionItem[];
}

function getReviewTasks(snapshot: DesktopStatusSnapshot) {
  return snapshot.latestTasks.filter((task) => {
    return (
      ["failed", "timed_out", "cancelled"].includes(task.status) ||
      Boolean(task.manualGateRequestId)
    );
  });
}

export function buildStatusOverview(
  snapshot: DesktopStatusSnapshot | null,
): StatusOverviewSummary {
  if (!snapshot) {
    return {
      postureLabel: "Waiting",
      postureDetail: "Waiting for the first desktop status snapshot.",
      postureTone: "neutral",
      successRateLabel: "N/A",
      failureDebt: 0,
      manualGateCount: 0,
      abnormalTaskCount: 0,
      browserTaskCount: 0,
      queuePressureLabel: "Unknown",
      queuePressureDetail: "Queue load will appear after the first refresh.",
      operationsHeadline: "Waiting for local control data",
      operationsDetail: "Refresh the desktop snapshot to build the local operator brief.",
      reviewTasks: [],
      reviewBuckets: [
        {
          id: "review-failure",
          label: "Failure debt",
          value: "0",
          detail: "No local queue history loaded yet.",
          tone: "neutral",
        },
        {
          id: "review-gates",
          label: "Manual gates",
          value: "0",
          detail: "Manual review load will appear after refresh.",
          tone: "neutral",
        },
        {
          id: "review-browser",
          label: "Browser lane",
          value: "0",
          detail: "No browser-task sample is available yet.",
          tone: "neutral",
        },
      ],
      metrics: [
        {
          label: "Queue posture",
          value: "Unknown",
          detail: "No queue snapshot has loaded yet.",
          tone: "neutral",
        },
        {
          label: "Running now",
          value: "0",
          detail: "Runtime task activity is not loaded yet.",
          tone: "neutral",
        },
        {
          label: "Failure debt",
          value: "0",
          detail: "Failure counters will appear after refresh.",
          tone: "neutral",
        },
        {
          label: "Manual gates",
          value: "0",
          detail: "Manual-review debt is waiting on the first snapshot.",
          tone: "neutral",
        },
        {
          label: "Browser lane",
          value: "Unknown",
          detail: "Browser task visibility is waiting on the desktop snapshot.",
          tone: "neutral",
        },
      ],
      attentionItems: [],
    };
  }

  const failureDebt =
    snapshot.counts.failed + snapshot.counts.timedOut + snapshot.counts.cancelled;
  const successRateLabel =
    snapshot.counts.total > 0
      ? `${Math.round((snapshot.counts.succeeded / snapshot.counts.total) * 100)}%`
      : "N/A";
  const manualGateCount = snapshot.latestTasks.filter((task) => task.manualGateRequestId).length;
  const abnormalTaskCount = snapshot.latestTasks.filter((task) =>
    ["failed", "timed_out", "cancelled"].includes(task.status),
  ).length;
  const browserTaskCount = snapshot.latestBrowserTasks.length;
  const reviewTasks = getReviewTasks(snapshot).slice(0, 4);
  const queuePressureRatio =
    snapshot.worker.workerCount > 0 ? snapshot.queueLen / snapshot.worker.workerCount : snapshot.queueLen;
  const backlogTone: StatusTone =
    snapshot.queueLen === 0
      ? "success"
      : snapshot.queueLen <= Math.max(1, snapshot.worker.workerCount)
        ? "neutral"
        : snapshot.queueLen <= Math.max(1, snapshot.worker.workerCount * 2)
          ? "warning"
          : "danger";
  const queuePressureLabel =
    backlogTone === "success"
      ? "Queue clear"
      : backlogTone === "neutral"
        ? "Within worker reach"
        : backlogTone === "warning"
          ? "Backlog rising"
          : "Queue pressure high";
  const queuePressureDetail =
    snapshot.worker.workerCount > 0
      ? `${snapshot.queueLen} queued tasks across ${snapshot.worker.workerCount} workers (${queuePressureRatio.toFixed(1)}x load).`
      : `${snapshot.queueLen} queued tasks with no worker count available.`;
  const postureLabel =
    backlogTone === "success"
      ? "Clear"
      : backlogTone === "neutral"
        ? "Contained"
        : backlogTone === "warning"
          ? "Rising"
          : "Pressed";
  const postureDetail =
    backlogTone === "success"
      ? "Queue is clear and the desktop is mostly operating on current work."
      : backlogTone === "neutral"
        ? "Queue is active but still within current worker capacity."
        : backlogTone === "warning"
          ? "Queue is growing faster than one worker cycle and should be watched."
          : "Queue pressure is above healthy range and deserves immediate operator attention.";
  const operationsHeadline =
    failureDebt > 0
      ? "Failure debt is the main operator burden."
      : manualGateCount > 0
        ? "Manual gates are the active review lane."
        : backlogTone === "danger"
          ? "Queue pressure needs a local operator reset."
          : backlogTone === "warning"
            ? "Queue is still serviceable, but drifting upward."
            : "Local queue posture is stable.";
  const operationsDetail =
    failureDebt > 0
      ? `${failureDebt} non-success outcomes are visible in the latest local sample, so cleanup and reruns should happen before new backlog hides them.`
      : manualGateCount > 0
        ? `${manualGateCount} recent tasks are carrying manual-gate markers, so the boss view should track human approval latency.`
        : `${snapshot.counts.running} tasks are active, ${snapshot.queueLen} remain queued, and the latest sample does not show review debt.`;
  const attentionItems: StatusAttentionItem[] = [];

  if (failureDebt > 0) {
    attentionItems.push({
      id: "failure-debt",
      tone: "warning",
      title: `${failureDebt} tasks still need review`,
      detail:
        "Failed, timed-out, and cancelled tasks are accumulating in the queue history and should be reconciled before they hide fresh issues.",
    });
  }

  if (snapshot.queueLen > Math.max(1, snapshot.worker.workerCount)) {
    attentionItems.push({
      id: "queue-pressure",
      tone: "warning",
      title: "Queue pressure is above worker count",
      detail: `${snapshot.queueLen} queued tasks vs ${snapshot.worker.workerCount} workers.`,
    });
  }

  if (manualGateCount > 0) {
    attentionItems.push({
      id: "manual-gate",
      tone: "info",
      title: `${manualGateCount} recent tasks are waiting on manual gates`,
      detail:
        "Manual gates are part of the current execution mix, so operators should keep an eye on tasks that need human confirmation.",
    });
  }

  if (browserTaskCount === 0 && snapshot.counts.total > 0) {
    attentionItems.push({
      id: "browser-lane",
      tone: "info",
      title: "No browser-task sample is visible in the latest feed",
      detail:
        "The snapshot is real and local, but the current browser-task lane is empty, so browser-side work may simply fall outside this sample window.",
    });
  }

  return {
    postureLabel,
    postureDetail,
    postureTone: backlogTone,
    successRateLabel,
    failureDebt,
    manualGateCount,
    abnormalTaskCount,
    browserTaskCount,
    queuePressureLabel,
    queuePressureDetail,
    operationsHeadline,
    operationsDetail,
    reviewTasks,
    reviewBuckets: [
      {
        id: "review-failure",
        label: "Failure debt",
        value: String(failureDebt),
        detail: `${abnormalTaskCount} abnormal tasks in the latest sample`,
        tone: failureDebt > 0 ? "danger" : "success",
      },
      {
        id: "review-gates",
        label: "Manual gates",
        value: String(manualGateCount),
        detail:
          manualGateCount > 0
            ? "Human approval is part of the current execution flow"
            : "Recent local tasks are not blocked on manual approval",
        tone: manualGateCount > 0 ? "warning" : "success",
      },
      {
        id: "review-browser",
        label: "Browser lane",
        value: String(browserTaskCount),
        detail:
          browserTaskCount > 0
            ? "Recent browser-oriented tasks are visible locally"
            : "No recent browser-task sample is visible locally",
        tone: browserTaskCount > 0 ? "neutral" : "warning",
      },
    ],
    metrics: [
      {
        label: "Queue posture",
        value: postureLabel,
        detail: `${snapshot.queueLen} queued / ${snapshot.worker.workerCount} workers`,
        tone: backlogTone,
      },
      {
        label: "Running now",
        value: String(snapshot.counts.running),
        detail: "Tasks actively executing right now",
        tone: snapshot.counts.running > 0 ? "success" : "neutral",
      },
      {
        label: "Failure debt",
        value: String(failureDebt),
        detail: `${snapshot.counts.failed} failed / ${snapshot.counts.timedOut} timed out / ${snapshot.counts.cancelled} cancelled`,
        tone: failureDebt > 0 ? "danger" : "success",
      },
      {
        label: "Manual gates",
        value: String(manualGateCount),
        detail:
          manualGateCount > 0
            ? "Recent tasks still depend on human confirmation"
            : "No manual-gate markers in the latest task sample",
        tone: manualGateCount > 0 ? "warning" : "success",
      },
      {
        label: "Browser lane",
        value: String(browserTaskCount),
        detail: `${snapshot.latestBrowserTasks.length} recent browser tasks in local sample`,
        tone: browserTaskCount > 0 ? "neutral" : "warning",
      },
    ],
    attentionItems: attentionItems.slice(0, 4),
  };
}
