import type { ProfileDataSource, ProfileDetail, ProfileDrawerTab } from "../../features/profiles/model";
import { formatRelativeTimestamp } from "../../utils/format";
import { EmptyState } from "../EmptyState";

const TABS: Array<{ value: ProfileDrawerTab; label: string }> = [
  { value: "overview", label: "Overview" },
  { value: "proxy", label: "Proxy" },
  { value: "runtime", label: "Runtime" },
  { value: "logs", label: "Logs" },
];

interface ProfileDrawerProps {
  openedProfileId: string | null;
  status: "idle" | "loading" | "ready" | "error";
  source: ProfileDataSource | null;
  detail: ProfileDetail | null;
  error: string | null;
  activeTab: ProfileDrawerTab;
  onClose: () => void;
  onRetry: () => void;
  onTabChange: (tab: ProfileDrawerTab) => void;
}

function formatListPreview(values: string[], max = 4): string {
  if (values.length === 0) {
    return "None";
  }
  if (values.length <= max) {
    return values.join(", ");
  }
  return `${values.slice(0, max).join(", ")} +${values.length - max}`;
}

function formatSchemaKind(value: string | null | undefined): string {
  if (!value) {
    return "Unknown";
  }
  return value
    .split(/[_\-\s]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function renderTabBody(activeTab: ProfileDrawerTab, detail: ProfileDetail) {
  const fingerprintSummary = detail.fingerprintSummary;
  const riskReasons = fingerprintSummary?.consistency?.riskReasons ?? [];
  const declaredSections = fingerprintSummary?.declaredSections ?? [];

  if (activeTab === "proxy") {
    return (
      <dl className="details-grid details-grid--stacked">
        <div className="details-grid__item">
          <dt>Provider</dt>
          <dd>{detail.proxyProvider ?? "No proxy linked"}</dd>
        </div>
        <div className="details-grid__item">
          <dt>Region / country</dt>
          <dd>
            {detail.proxyRegion ?? "Pending"}
            <br />
            {detail.proxyCountry ?? "Pending"}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Resolution / mode</dt>
          <dd>
            {detail.proxyResolutionStatus ?? "Unknown"}
            <br />
            {detail.proxyUsageMode ?? "Unknown"}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Residency / rotation</dt>
          <dd>
            {detail.proxyResidencyStatus ?? "Unknown"}
            <br />
            {detail.proxyRotationMode ?? "Unknown"}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Session / sticky expiry</dt>
          <dd>
            {detail.proxySessionKey ?? "none"}
            <br />
            {formatRelativeTimestamp(detail.proxyExpiresAt ?? null)}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Requested provider / region</dt>
          <dd>
            {detail.proxyRequestedProvider ?? "inherit-provider"}
            <br />
            {detail.proxyRequestedRegion ?? "inherit-region"}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Last verify / use</dt>
          <dd>
            {formatRelativeTimestamp(detail.proxyLastVerifiedAt)}
            <br />
            {formatRelativeTimestamp(detail.proxyLastUsedAt)}
          </dd>
        </div>
      </dl>
    );
  }

  if (activeTab === "runtime") {
    return (
      <div className="profile-drawer__list">
        <dl className="details-grid details-grid--stacked">
          <div className="details-grid__item">
            <dt>Status</dt>
            <dd>
              {detail.profile.runtimeStatus}
              <br />
              {detail.activeSessionCount} active sessions / {detail.profile.pendingActionCount} pending actions
            </dd>
          </div>
          <div className="details-grid__item">
            <dt>Continuity</dt>
            <dd>
              {detail.continuityStatus ?? "Unknown"}
              <br />
              Score {detail.continuityScore ?? "N/A"} / risks {detail.loginRiskCount}
            </dd>
          </div>
          <div className="details-grid__item">
            <dt>Last activity</dt>
            <dd>
              Task {formatRelativeTimestamp(detail.profile.lastActiveAt)}
              <br />
              Opened {formatRelativeTimestamp(detail.profile.lastOpenedAt)}
            </dd>
          </div>
          <div className="details-grid__item">
            <dt>Consumption status</dt>
            <dd>
              {fingerprintSummary?.consumption?.consumptionStatus ?? "Unknown"}
              <br />
              Declared {fingerprintSummary?.consumption?.declaredCount ?? "N/A"} / Applied{" "}
              {fingerprintSummary?.consumption?.appliedCount ?? "N/A"} / Ignored{" "}
              {fingerprintSummary?.consumption?.ignoredCount ?? "N/A"}
            </dd>
          </div>
          <div className="details-grid__item">
            <dt>Declared vs runtime</dt>
            <dd>
              Supported {fingerprintSummary?.supportedRuntimeFields.length ?? 0}
              <br />
              Unsupported {fingerprintSummary?.unsupportedControlFields.length ?? 0}
            </dd>
          </div>
          <div className="details-grid__item">
            <dt>Supported field preview</dt>
            <dd>{formatListPreview(fingerprintSummary?.supportedRuntimeFields ?? [])}</dd>
          </div>
          <div className="details-grid__item">
            <dt>Ignored field preview</dt>
            <dd>
              {formatListPreview(
                fingerprintSummary?.consumption?.ignoredFields ??
                  fingerprintSummary?.unsupportedControlFields ??
                  [],
              )}
            </dd>
          </div>
          <div className="details-grid__item">
            <dt>Runtime warning</dt>
            <dd>{fingerprintSummary?.consumption?.partialSupportWarning ?? "None"}</dd>
          </div>
        </dl>

        <div className="record-list">
          {detail.recentTasks.length === 0 ? (
            <EmptyState
              title="No recent tasks"
              detail="Recent task history will show here when the desktop detail contract returns task rows."
            />
          ) : (
            detail.recentTasks.map((task) => (
              <article className="record-card record-card--compact" key={task.id}>
                <div className="record-card__top">
                  <div>
                    <strong>{task.title}</strong>
                    <p className="record-card__subline">{task.id}</p>
                  </div>
                  <span className="badge">{task.status}</span>
                </div>
                <div className="record-card__footer">
                  <span>Created {formatRelativeTimestamp(task.createdAt)}</span>
                  <span>Finished {formatRelativeTimestamp(task.finishedAt)}</span>
                </div>
              </article>
            ))
          )}
        </div>
      </div>
    );
  }

  if (activeTab === "logs") {
    return (
      <div className="record-list">
        {detail.recentLogs.length === 0 ? (
          <EmptyState
            title="No recent logs"
            detail="Profile-scoped logs will appear here once the detail reader returns log rows."
          />
        ) : (
          detail.recentLogs.map((log) => (
            <article className="record-card record-card--compact" key={log.id}>
              <div className="record-card__top">
                <div>
                  <strong>{log.message}</strong>
                  <p className="record-card__subline">{log.id}</p>
                </div>
                <span className={`badge badge--${log.level.toLowerCase()}`}>{log.level}</span>
              </div>
              <div className="record-card__footer">
                <span>{formatRelativeTimestamp(log.createdAt)}</span>
                <span>Profile detail log stream</span>
              </div>
            </article>
          ))
        )}
      </div>
    );
  }

  return (
    <div className="profile-drawer__list">
      <dl className="details-grid details-grid--stacked">
        <div className="details-grid__item">
          <dt>Store / platform</dt>
          <dd>
            {detail.profile.storeId}
            <br />
            {detail.profile.platformLabel}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Status / groups</dt>
          <dd>
            {detail.profile.statusLabel}
            <br />
            {detail.profile.groupLabels.join(", ")}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Locale / timezone</dt>
          <dd>
            {detail.profile.localeLabel}
            <br />
            {detail.profile.timezoneLabel}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Fingerprint family / schema</dt>
          <dd>
            {fingerprintSummary?.familyId ?? detail.fingerprintProfileLabel}
            <br />
            {formatSchemaKind(fingerprintSummary?.schemaKind)}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Coherence / status</dt>
          <dd>
            {fingerprintSummary?.consistency?.coherenceScore ?? "N/A"}
            <br />
            {formatSchemaKind(fingerprintSummary?.consistency?.overallStatus)}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Risk reasons</dt>
          <dd>{riskReasons.length > 0 ? formatListPreview(riskReasons, 2) : "No major risk reason"}</dd>
        </div>
        <div className="details-grid__item">
          <dt>Declared controls</dt>
          <dd>
            {fingerprintSummary?.declaredControlCount ?? "N/A"}
            <br />
            {formatListPreview(fingerprintSummary?.declaredControlFields ?? [])}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Sections coverage</dt>
          <dd>
            {declaredSections.length > 0
              ? declaredSections
                  .map((section) => `${section.name}(${section.declaredCount})`)
                  .join(", ")
              : "No declared sections"}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Validation / issues</dt>
          <dd>
            {fingerprintSummary?.validationOk == null
              ? "Unknown"
              : fingerprintSummary.validationOk
                ? "Passed"
                : "Failed"}
            <br />
            {formatListPreview(fingerprintSummary?.validationIssues ?? [], 2)}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Behavior profile</dt>
          <dd>
            {detail.behaviorProfileLabel ?? "Not linked"}
            <br />
            {fingerprintSummary
              ? `Summary source: ${fingerprintSummary.source}`
              : "Fingerprint summary not available"}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Policies</dt>
          <dd>
            {detail.networkPolicyLabel}
            <br />
            {detail.continuityPolicyLabel}
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Tags</dt>
          <dd>{detail.profile.tags.join(", ") || "No tags"}</dd>
        </div>
      </dl>
    </div>
  );
}

export function ProfileDrawer({
  openedProfileId,
  status,
  source,
  detail,
  error,
  activeTab,
  onClose,
  onRetry,
  onTabChange,
}: ProfileDrawerProps) {
  return (
    <aside className="panel profile-drawer">
      <header className="panel__header">
        <div>
          <span className="shell__eyebrow">Details Drawer</span>
          <h2 className="panel__title">
            {detail?.profile.name ?? (openedProfileId ? "Loading profile" : "Profile detail")}
          </h2>
          <p className="panel__subtitle">
            {source ? `Data source: ${source}` : "Open a profile row to inspect detail."}
          </p>
        </div>
        {openedProfileId ? (
          <button className="button button--secondary" type="button" onClick={onClose}>
            Close
          </button>
        ) : null}
      </header>

      {!openedProfileId ? (
        <EmptyState
          title="Drawer ready"
          detail="Click a profile row or use Inspect to open the right-side detail drawer."
        />
      ) : null}

      {openedProfileId && status === "loading" ? (
        <div className="empty-state">
          <strong>Loading profile detail...</strong>
          <p>Waiting for desktop detail reader or fallback adapter.</p>
        </div>
      ) : null}

      {openedProfileId && status === "error" ? (
        <div className="profile-drawer__error">
          <div className="banner banner--error">{error}</div>
          <button className="button button--secondary" type="button" onClick={onRetry}>
            Retry
          </button>
        </div>
      ) : null}

      {openedProfileId && status === "ready" && detail ? (
        <div className="profile-drawer__body">
          <div className="profiles-toolbar__columns">
            {TABS.map((tab) => (
              <button
                key={tab.value}
                className={`profiles-toolbar__column-chip ${
                  activeTab === tab.value ? "is-active" : ""
                }`}
                type="button"
                onClick={() => onTabChange(tab.value)}
              >
                {tab.label}
              </button>
            ))}
          </div>

          {renderTabBody(activeTab, detail)}
        </div>
      ) : null}
    </aside>
  );
}
