import type { DesktopRuntimeStatus } from "../../types/desktop";

type RuntimeTone = "neutral" | "success" | "warning" | "danger";
type RuntimeNoticeTone = "info" | "warning" | "error";

export interface RuntimeAttentionItem {
  id: string;
  tone: RuntimeNoticeTone;
  title: string;
  detail: string;
}

export interface RuntimePostureFact {
  id: string;
  label: string;
  value: string;
  detail: string;
  tone: RuntimeTone;
}

export interface RuntimeOverviewSummary {
  postureLabel: string;
  postureDetail: string;
  postureTone: RuntimeTone;
  ownershipLabel: string;
  ownershipDetail: string;
  healthLabel: string;
  healthDetail: string;
  cadenceLabel: string;
  cadenceDetail: string;
  controlLabel: string;
  controlDetail: string;
  runtimeAgeLabel: string;
  pathCoverageLabel: string;
  pathCoverageDetail: string;
  postureFacts: RuntimePostureFact[];
  attentionItems: RuntimeAttentionItem[];
}

function getRuntimeAgeLabel(startedAt: string | null) {
  if (!startedAt) {
    return "No start marker";
  }

  const parsed = Date.parse(startedAt);
  if (Number.isNaN(parsed)) {
    return startedAt;
  }

  const deltaSeconds = Math.max(0, Math.floor((Date.now() - parsed) / 1000));
  if (deltaSeconds < 60) {
    return `${deltaSeconds}s active`;
  }
  if (deltaSeconds < 3600) {
    return `${Math.floor(deltaSeconds / 60)}m active`;
  }
  if (deltaSeconds < 86400) {
    return `${Math.floor(deltaSeconds / 3600)}h active`;
  }
  return `${Math.floor(deltaSeconds / 86400)}d active`;
}

export function buildRuntimeOverview(
  snapshot: DesktopRuntimeStatus | null,
  activeAction: "start" | "stop" | null,
  autoRefreshEnabled: boolean,
  refreshIntervalMs: number,
): RuntimeOverviewSummary {
  const attentionItems: RuntimeAttentionItem[] = [];
  const cadenceLabel = autoRefreshEnabled
    ? `${Math.round(refreshIntervalMs / 1000)}s runtime poll`
    : "Manual runtime refresh";
  const cadenceDetail = autoRefreshEnabled
    ? "Runtime status keeps polling while this view stays open."
    : "Runtime status only changes when an operator refreshes it.";

  if (!snapshot) {
    return {
      postureLabel: activeAction ? "Applying" : "Waiting",
      postureDetail: activeAction
        ? `Runtime ${activeAction} command is in flight.`
        : "Waiting for the first runtime snapshot from the local desktop service.",
      postureTone: "neutral",
      ownershipLabel: "Unknown",
      ownershipDetail: "Runtime ownership has not been confirmed yet.",
      healthLabel: "No health signal",
      healthDetail: "The health endpoint has not responded yet.",
      cadenceLabel,
      cadenceDetail,
      controlLabel: "Control unknown",
      controlDetail: "The desktop has not confirmed whether it owns the runtime yet.",
      runtimeAgeLabel: "Not running",
      pathCoverageLabel: "Path coverage unknown",
      pathCoverageDetail: "Binary and log path disclosure will appear once the runtime snapshot loads.",
      postureFacts: [
        {
          id: "runtime-control",
          label: "Control",
          value: "Unknown",
          detail: "Waiting for runtime ownership details.",
          tone: "neutral",
        },
        {
          id: "runtime-health",
          label: "Health",
          value: "Unknown",
          detail: "No health endpoint response yet.",
          tone: "neutral",
        },
        {
          id: "runtime-paths",
          label: "Artifacts",
          value: "Unknown",
          detail: "Log and stdio path visibility is waiting on runtime snapshot.",
          tone: "neutral",
        },
      ],
      attentionItems,
    };
  }

  let postureLabel = "Healthy";
  let postureDetail = "Runtime is reachable and under local control.";
  let postureTone: RuntimeTone = "success";

  if (activeAction) {
    postureLabel = "Applying";
    postureDetail = `Runtime ${activeAction} command is in flight.`;
    postureTone = "neutral";
  } else if (!snapshot.running) {
    postureLabel = "Stopped";
    postureDetail = "Runtime is not running, so new local automation work cannot execute.";
    postureTone = "warning";
  } else if (!snapshot.apiReachable) {
    postureLabel = "Starting";
    postureDetail = "Runtime process exists, but the health endpoint is not ready yet.";
    postureTone = "warning";
  } else if (snapshot.status === "external_running") {
    postureLabel = "External";
    postureDetail = "Runtime is healthy, but it is running outside the desktop controller.";
    postureTone = "neutral";
  }

  if (snapshot.status === "external_running") {
    attentionItems.push({
      id: "external",
      tone: "info",
      title: "Runtime ownership is external",
      detail:
        "The process is alive, but it was started outside the desktop controller. Stop and restart locally if you need full operator ownership.",
    });
  }

  if (snapshot.running && !snapshot.apiReachable) {
    attentionItems.push({
      id: "health",
      tone: "warning",
      title: "Health endpoint is not ready",
      detail:
        "The runtime process is present, but the local API is not reachable yet. Watch startup before sending more work.",
    });
  }

  if (!snapshot.running && snapshot.lastExitCode !== null) {
    attentionItems.push({
      id: "exit-code",
      tone: "error",
      title: `Last managed runtime exited with code ${snapshot.lastExitCode}`,
      detail:
        "Use the runtime detail cards below to inspect paths, ownership, and recovery posture before restarting.",
    });
  }

  if (!snapshot.logDir && snapshot.running) {
    attentionItems.push({
      id: "log-dir",
      tone: "info",
      title: "Runtime log directory is not reported",
      detail: "The runtime is running, but no log directory path was exposed in the current snapshot.",
    });
  }

  const pathCoverageCount = [
    snapshot.binaryPath,
    snapshot.logDir,
    snapshot.stdoutPath,
    snapshot.stderrPath,
  ].filter(Boolean).length;
  const pathCoverageLabel =
    pathCoverageCount >= 4
      ? "Full local path coverage"
      : pathCoverageCount >= 2
        ? "Partial path coverage"
        : "Thin path coverage";
  const pathCoverageDetail = `${pathCoverageCount}/4 local runtime artifact paths are exposed.`;
  const controlLabel = snapshot.managed
    ? "Desktop-owned"
    : snapshot.running
      ? "Externally owned"
      : "Ready for local ownership";
  const controlDetail = snapshot.managed
    ? "Start and stop controls map to the managed local runtime."
    : snapshot.running
      ? "The runtime is alive, but not owned by the desktop controller."
      : "No process is running, so the next start will create a managed local runtime.";

  return {
    postureLabel,
    postureDetail,
    postureTone,
    ownershipLabel: snapshot.managed ? "Managed by desktop" : "External owner",
    ownershipDetail: snapshot.pid ? `PID ${snapshot.pid}` : "No active PID reported",
    healthLabel: snapshot.apiReachable ? "Health reachable" : "Health pending",
    healthDetail: snapshot.healthUrl,
    cadenceLabel,
    cadenceDetail,
    controlLabel,
    controlDetail,
    runtimeAgeLabel: snapshot.running ? getRuntimeAgeLabel(snapshot.startedAt) : "Not running",
    pathCoverageLabel,
    pathCoverageDetail,
    postureFacts: [
      {
        id: "runtime-control",
        label: "Control",
        value: controlLabel,
        detail: controlDetail,
        tone: snapshot.managed ? "success" : snapshot.running ? "warning" : "neutral",
      },
      {
        id: "runtime-health",
        label: "Health",
        value: snapshot.apiReachable ? "Reachable" : "Pending",
        detail: snapshot.healthUrl,
        tone: snapshot.apiReachable ? "success" : snapshot.running ? "warning" : "neutral",
      },
      {
        id: "runtime-paths",
        label: "Artifacts",
        value: pathCoverageLabel,
        detail: pathCoverageDetail,
        tone:
          pathCoverageCount >= 4
            ? "success"
            : pathCoverageCount >= 2
              ? "warning"
              : "neutral",
      },
    ],
    attentionItems,
  };
}
