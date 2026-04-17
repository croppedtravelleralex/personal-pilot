import { EmptyState } from "../EmptyState";
import { InlineContentPreview } from "../InlineContentPreview";
import { Panel } from "../Panel";
import type {
  AutomationNoticeTone,
  AutomationLauncherDraft,
  PreparedLaunchPlan,
} from "../../features/automation/model";
import type { AutomationTemplateRecommendation } from "../../features/automation/derived";
import type { RecorderSessionModel } from "../../features/recorder/model";
import type {
  TemplateBindingDraft,
  TemplateCompileRequestDraft,
  TemplateSummary,
  TemplateVariable,
} from "../../features/templates/model";
import type { DesktopTaskItem } from "../../types/desktop";
import { formatRelativeTimestamp, formatStatusLabel } from "../../utils/format";

const MODE_OPTIONS = [
  { value: "queue", label: "Queue for execution" },
  { value: "dry_run", label: "Dry run preflight" },
  { value: "batch_prepare", label: "Batch staging" },
] as const;

const TARGET_SCOPE_OPTIONS = [
  { value: "template_default", label: "Template default targets" },
  { value: "selected_profile_group", label: "Selected profile group" },
  { value: "last_active_profile", label: "Last active profile" },
] as const;

export interface RunLauncherLaunchResult {
  status: "ready" | "queued" | "running" | "failed" | "blocked" | string;
  headline: string;
  detail: string;
  launchedAtLabel?: string | null;
  queueLabel?: string | null;
  acceptedProfileCount?: number | null;
  runId?: string | null;
  warnings?: string[];
}

interface RunLauncherProps {
  templates: TemplateSummary[];
  selectedTemplateId: string | null;
  selectedTemplate: TemplateSummary | null;
  bindingDraft: TemplateBindingDraft | null;
  compileDraft: TemplateCompileRequestDraft | null;
  draft: AutomationLauncherDraft;
  launcherNotice: string | null;
  launcherNoticeTone: AutomationNoticeTone;
  lastPreparedLaunch: PreparedLaunchPlan | null;
  isPreparingLaunch: boolean;
  selectedRun: DesktopTaskItem | null;
  recorderSnapshot: RecorderSessionModel | null;
  recommendation: AutomationTemplateRecommendation | null;
  isLaunching?: boolean;
  launchNotice?: string | null;
  launchNoticeTone?: AutomationNoticeTone;
  lastLaunchResult?: RunLauncherLaunchResult | null;
  onSelectTemplate: (templateId: string) => void;
  onSetMode: (mode: AutomationLauncherDraft["mode"]) => void;
  onSetTargetScope: (targetScope: AutomationLauncherDraft["targetScope"]) => void;
  onSetLaunchNote: (value: string) => void;
  onSetBindingValue: (variableKey: string, value: string) => void;
  onSetBindingNote: (value: string) => void;
  onSetBindingProfileIdsInput: (value: string) => void;
  onResetBindings: () => void;
  onPrepareLaunch: () => void;
  onResetLaunch: () => void;
  onLaunch?: () => void;
}

function getNoticeClassName(tone: AutomationNoticeTone) {
  if (tone === "error") {
    return "banner banner--error";
  }

  if (tone === "success" || tone === "info") {
    return "banner banner--info";
  }

  return "banner banner--warning";
}

function renderVariableField(
  variable: TemplateVariable,
  value: string,
  onChange: (value: string) => void,
) {
  if (variable.kind === "textarea") {
    return (
      <textarea
        className="field__textarea"
        value={value}
        placeholder={variable.example}
        onChange={(event) => onChange(event.target.value)}
      />
    );
  }

  return (
    <input
      className="field__input"
      type={variable.kind === "number" || variable.kind === "duration" ? "number" : "text"}
      value={value}
      placeholder={variable.example}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}

function getPostureTone(
  canLaunch: boolean,
  isPreparingLaunch: boolean,
  isLaunching: boolean,
): "badge--info" | "badge--warning" {
  if (canLaunch || isPreparingLaunch || isLaunching) {
    return "badge--info";
  }

  return "badge--warning";
}

export function RunLauncher({
  templates,
  selectedTemplateId,
  selectedTemplate,
  bindingDraft,
  compileDraft,
  draft,
  launcherNotice,
  launcherNoticeTone,
  lastPreparedLaunch,
  isPreparingLaunch,
  selectedRun,
  recorderSnapshot,
  recommendation,
  isLaunching = false,
  launchNotice = null,
  launchNoticeTone = "info",
  lastLaunchResult = null,
  onSelectTemplate,
  onSetMode,
  onSetTargetScope,
  onSetLaunchNote,
  onSetBindingValue,
  onSetBindingNote,
  onSetBindingProfileIdsInput,
  onResetBindings,
  onPrepareLaunch,
  onResetLaunch,
  onLaunch,
}: RunLauncherProps) {
  const compileSourceSummary = compileDraft
    ? `${compileDraft.targetSource} target / ${compileDraft.recorderSource} recorder`
    : "waiting for manifest context";
  const canLaunch = Boolean(lastPreparedLaunch?.ready && onLaunch && !isPreparingLaunch);
  const launchStateLabel = isLaunching
    ? "dispatching"
    : lastLaunchResult?.status ?? (lastPreparedLaunch?.ready ? "ready to launch" : "prepare required");
  const preflightChecks = [
    {
      label: "Source run",
      status: selectedRun ? "ready" : "blocked",
      detail: selectedRun
        ? `${selectedRun.title ?? selectedRun.kind} / ${selectedRun.personaId ?? "persona pending"}`
        : "Select a local run so persona and platform context can be resolved.",
    },
    {
      label: "Template",
      status: selectedTemplate ? "ready" : "blocked",
      detail: selectedTemplate
        ? `${selectedTemplate.name} / ${selectedTemplate.status} / ${selectedTemplate.platformId}`
        : "Choose a template before binding variables and reviewing compile posture.",
    },
    {
      label: "Bindings",
      status: compileDraft?.missingRequiredKeys.length ? "blocked" : compileDraft ? "ready" : "pending",
      detail:
        compileDraft?.missingRequiredKeys.length
          ? `Missing required: ${compileDraft.missingRequiredKeys.join(", ")}`
          : bindingDraft
            ? `${bindingDraft.completionCount} fields complete, ${bindingDraft.missingRequiredCount} required still missing.`
            : "Binding draft will appear after template selection.",
    },
    {
      label: "Targets",
      status:
        compileDraft && compileDraft.targetProfileIds.length > 0
          ? "ready"
          : compileDraft
            ? "blocked"
            : "pending",
      detail:
        compileDraft?.targetProfileIds.length
          ? compileDraft.targetProfileIds.join(", ")
          : "Bind explicit profile ids or reuse the selected run persona.",
    },
    {
      label: "Recorder",
      status:
        recorderSnapshot?.source === "desktop"
          ? "ready"
          : recorderSnapshot
            ? "review"
            : "pending",
      detail: recorderSnapshot
        ? `${recorderSnapshot.source} / ${recorderSnapshot.status} / ${recorderSnapshot.stepCount} steps`
        : "Recorder context is optional, but step evidence and variable hydration stay thinner without it.",
    },
    {
      label: "Compile manifest",
      status:
        lastPreparedLaunch?.compilePreview.status ?? (compileDraft?.ready ? "pending" : "blocked"),
      detail:
        lastPreparedLaunch?.compilePreview.message ??
        (compileDraft?.ready
          ? "Prepare to write the local manifest and verify accepted targets."
          : "Compile preflight is blocked until required bindings and targets are clean."),
    },
  ];

  return (
    <Panel
      title="Launch Console"
      subtitle="Review preflight, confirm dispatch posture, and move a prepared local manifest into local runtime execution through the connected desktop launch contract."
      actions={
        <span className={`badge ${getPostureTone(canLaunch, isPreparingLaunch, isLaunching)}`}>
          {isPreparingLaunch
            ? "Preparing"
            : isLaunching
              ? "Dispatching"
              : canLaunch
                ? "Dispatch ready"
                : compileDraft?.ready
                  ? "Prepared state"
                  : "Preflight blocked"}
        </span>
      }
    >
      <div className="page-stack">
        <div className="automation-metric-strip automation-metric-strip--compact">
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Run context</span>
            <strong>{selectedRun?.title ?? selectedRun?.kind ?? "No run selected"}</strong>
            <small>
              {selectedRun
                ? `${selectedRun.platformId ?? "platform?"} / ${selectedRun.personaId ?? "persona?"}`
                : "Waiting for task inventory selection"}
            </small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Recorder posture</span>
            <strong>{recorderSnapshot?.status ?? "no session"}</strong>
            <small>
              {recorderSnapshot
                ? `${recorderSnapshot.source} / ${recorderSnapshot.stepCount} steps`
                : "Optional but useful for evidence and variable hydration"}
            </small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Dispatch posture</span>
            <strong>{launchStateLabel}</strong>
            <small>
              {lastLaunchResult?.queueLabel ??
                (lastPreparedLaunch?.ready
                  ? "Prepared launch can move into local runtime dispatch."
                  : "Prepare once compile preflight is clean.")}
            </small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Template guidance</span>
            <strong>{recommendation?.templateName ?? "Manual selection"}</strong>
            <small>{recommendation?.reason ?? "No automatic recommendation yet."}</small>
          </article>
        </div>

        <div className="details-grid details-grid--two">
          {preflightChecks.map((check) => (
            <article className="contract-card" key={check.label}>
              <div className="contract-card__top">
                <strong>{check.label}</strong>
                <span
                  className={`badge ${
                    check.status === "ready"
                      ? "badge--info"
                      : check.status === "review" || check.status === "pending"
                        ? "badge--warning"
                        : "badge--failed"
                  }`}
                >
                  {formatStatusLabel(check.status)}
                </span>
              </div>
              <p>{check.detail}</p>
            </article>
          ))}
        </div>

        <div className="banner banner--warning">
          <strong>Reality boundary</strong>
          <div>
            This panel stages and dispatches the local compile/launch loop that exists today. It
            is not an AdsPower-grade orchestration plane with vendor-level control towers or
            multi-branch debugging beyond the desktop commands currently wired here.
          </div>
        </div>

        <label className="field">
          <span className="field__label">Selected template</span>
          <select
            className="field__input"
            value={selectedTemplateId ?? ""}
            onChange={(event) => onSelectTemplate(event.target.value)}
          >
            {templates.map((template) => (
              <option key={template.id} value={template.id}>
                {template.name}
              </option>
            ))}
          </select>
        </label>

        <div className="details-grid details-grid--two">
          <label className="field">
            <span className="field__label">Launch mode</span>
            <select
              className="field__input"
              value={draft.mode}
              onChange={(event) =>
                onSetMode(event.target.value as AutomationLauncherDraft["mode"])
              }
            >
              {MODE_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span className="field__label">Target scope</span>
            <select
              className="field__input"
              value={draft.targetScope}
              onChange={(event) =>
                onSetTargetScope(
                  event.target.value as AutomationLauncherDraft["targetScope"],
                )
              }
            >
              {TARGET_SCOPE_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
        </div>

        {!selectedTemplate || !bindingDraft ? (
          <EmptyState
            title="No template selected"
            detail="Choose a template to confirm variable bindings, prepare launch context, and unlock the dispatch lane."
          />
        ) : (
          <>
            <div className="details-grid details-grid--two">
              <article className="details-grid__item">
                <dt>Template status</dt>
                <dd>{selectedTemplate.status}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Readiness / source</dt>
                <dd>
                  {selectedTemplate.readinessLevel}
                  <br />
                  {selectedTemplate.sourceLabel}
                </dd>
              </article>
              <article className="details-grid__item">
                <dt>Binding progress</dt>
                <dd>
                  {bindingDraft.completionCount} completed
                  <br />
                  {bindingDraft.missingRequiredCount} missing required
                </dd>
              </article>
              <article className="details-grid__item">
                <dt>Execution outline</dt>
                <dd>
                  {selectedTemplate.stepCount} template steps
                  <br />
                  {selectedTemplate.variableCount} variables
                </dd>
              </article>
              <article className="details-grid__item">
                <dt>Run alignment</dt>
                <dd>
                  {selectedRun?.platformId === selectedTemplate.platformId
                    ? "Platform aligned"
                    : "Platform differs"}
                  <br />
                  {recommendation?.templateId === selectedTemplate.id
                    ? "Current template is recommended"
                    : "Manual override in use"}
                </dd>
              </article>
              <article className="details-grid__item">
                <dt>Allowed regions / compiler</dt>
                <dd>
                  {selectedTemplate.allowedRegions.length > 0
                    ? selectedTemplate.allowedRegions.join(", ")
                    : "No explicit region limit"}
                  <br />
                  {selectedTemplate.compilerState}
                </dd>
              </article>
            </div>

            <div className="launcher-bindings">
              {selectedTemplate.variables.map((variable) => {
                const bindingValue = bindingDraft.values[variable.key];

                return (
                  <label className="field" key={variable.key}>
                    <span className="field__label">
                      {variable.label}
                      {variable.required ? " *" : ""}
                    </span>
                    {renderVariableField(variable, bindingValue?.value ?? "", (value) =>
                      onSetBindingValue(variable.key, value),
                    )}
                    <span className="field__hint">
                      {bindingValue?.source ?? "empty"} | {variable.source}
                      {bindingValue?.error ? ` | ${bindingValue.error}` : ""}
                    </span>
                  </label>
                );
              })}
            </div>

            <label className="field">
              <span className="field__label">Target profile ids</span>
              <input
                className="field__input"
                type="text"
                value={bindingDraft.profileIdsInput}
                placeholder="profile-a,profile-b"
                onChange={(event) => onSetBindingProfileIdsInput(event.target.value)}
              />
              <span className="field__hint">
                Leave empty to fall back to the selected run profile when available.
              </span>
            </label>

            <label className="field">
              <span className="field__label">Binding note / compile hints</span>
              <textarea
                className="field__textarea"
                value={bindingDraft.note}
                placeholder="Explain variable defaults, compile hints, or replay notes."
                onChange={(event) => onSetBindingNote(event.target.value)}
              />
            </label>

            <label className="field">
              <span className="field__label">Operator launch note</span>
              <textarea
                className="field__textarea"
                value={draft.launchNote}
                placeholder="Add queue notes, risk flags, or operator instructions for this launch."
                onChange={(event) => onSetLaunchNote(event.target.value)}
              />
            </label>
          </>
        )}

        {compileDraft ? (
          <article className="record-card record-card--compact launcher-preview">
            <div className="record-card__top">
              <strong>Compile preflight</strong>
              <span className={`badge ${compileDraft.ready ? "badge--info" : "badge--warning"}`}>
                {compileDraft.ready ? "Ready" : "Blocked"}
              </span>
            </div>
            <div className="record-card__meta">
              <span>{compileDraft.stepCount} template steps</span>
              <span>{compileDraft.recorderStepCount} recorder steps</span>
              <span>{compileDraft.bindings.length} bindings</span>
              <span>{compileDraft.targetProfileIds.length} target profiles</span>
            </div>
            <p className="record-card__content record-card__content--muted">
              {compileSourceSummary}
            </p>
            {compileDraft.missingRequiredKeys.length > 0 ? (
              <p className="record-card__content">
                Missing required: {compileDraft.missingRequiredKeys.join(", ")}
              </p>
            ) : null}
            {compileDraft.warnings.length > 0 ? (
              <div className="automation-pill-list">
                {compileDraft.warnings.map((warning) => (
                  <span className="automation-pill" key={warning}>
                    {warning}
                  </span>
                ))}
              </div>
            ) : null}
            <div className="record-card__footer">
              <span>
                Effective targets:{" "}
                {compileDraft.targetProfileIds.length > 0
                  ? compileDraft.targetProfileIds.join(", ")
                  : "none"}
              </span>
              <span>Generated {formatRelativeTimestamp(compileDraft.generatedAt)}</span>
            </div>
          </article>
        ) : null}

        {launcherNotice ? (
          <div className={getNoticeClassName(launcherNoticeTone)}>
            <InlineContentPreview value={launcherNotice} collapseAt={240} inlineLimit={6000} />
          </div>
        ) : null}

        {lastPreparedLaunch ? (
          <article className="record-card record-card--compact launcher-preview">
            <div className="record-card__top">
              <strong>Prepared launch package</strong>
              <span className={`badge ${lastPreparedLaunch.ready ? "badge--info" : "badge--warning"}`}>
                {lastPreparedLaunch.ready ? "Launchable" : lastPreparedLaunch.mode}
              </span>
            </div>
            <div className="record-card__meta">
              <span>{lastPreparedLaunch.templateName}</span>
              <span>{lastPreparedLaunch.targetScope}</span>
              <span>{lastPreparedLaunch.boundProfileIds.length} target profiles</span>
              <span>{lastPreparedLaunch.recorderStepCount} recorder steps</span>
            </div>
            <InlineContentPreview
              className="record-card__content"
              value={lastPreparedLaunch.compilePreview.message}
              collapseAt={240}
              inlineLimit={10000}
            />
            <div className="details-grid details-grid--two">
              <article className="details-grid__item">
                <dt>Prepared at</dt>
                <dd>{lastPreparedLaunch.preparedAtLabel}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Compile stamp</dt>
                <dd>{lastPreparedLaunch.compilePreview.compiledAtLabel ?? "Not returned"}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Source run</dt>
                <dd>{lastPreparedLaunch.sourceRunId ?? "Not bound"}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Recorder session</dt>
                <dd>{lastPreparedLaunch.recorderSessionId ?? "No recorder session"}</dd>
              </article>
            </div>
            {lastPreparedLaunch.compilePreview.blockers.length > 0 ? (
              <div className="automation-pill-list">
                {lastPreparedLaunch.compilePreview.blockers.map((item) => (
                  <span className="automation-pill" key={item}>
                    {item}
                  </span>
                ))}
              </div>
            ) : null}
            {lastPreparedLaunch.compilePreview.warnings.length > 0 ? (
              <div className="automation-pill-list">
                {lastPreparedLaunch.compilePreview.warnings.map((item) => (
                  <span className="automation-pill" key={item}>
                    {item}
                  </span>
                ))}
              </div>
            ) : null}
            {lastPreparedLaunch.note ? (
              <InlineContentPreview
                className="record-card__content"
                bodyClassName="record-card__content--muted"
                value={`Operator note: ${lastPreparedLaunch.note}`}
                collapseAt={220}
                inlineLimit={8000}
                muted
              />
            ) : null}
          </article>
        ) : null}

        {launchNotice ? (
          <div className={getNoticeClassName(launchNoticeTone)}>
            <InlineContentPreview value={launchNotice} collapseAt={240} inlineLimit={6000} />
          </div>
        ) : null}

        {lastLaunchResult ? (
          <article className="record-card record-card--compact launcher-preview">
            <div className="record-card__top">
              <strong>Dispatch result</strong>
              <span className={`badge badge--${lastLaunchResult.status}`}>{lastLaunchResult.status}</span>
            </div>
            <InlineContentPreview
              className="record-card__content"
              value={lastLaunchResult.headline}
              collapseAt={220}
              inlineLimit={8000}
            />
            <InlineContentPreview
              className="record-card__content"
              bodyClassName="record-card__content--muted"
              value={lastLaunchResult.detail}
              collapseAt={240}
              inlineLimit={10000}
              muted
            />
            <div className="details-grid details-grid--two">
              <article className="details-grid__item">
                <dt>Run</dt>
                <dd>{lastLaunchResult.runId ?? "Not returned"}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Accepted targets</dt>
                <dd>
                  {lastLaunchResult.acceptedProfileCount != null
                    ? String(lastLaunchResult.acceptedProfileCount)
                    : "Runtime counters pending"}
                </dd>
              </article>
              <article className="details-grid__item">
                <dt>Launch stamp</dt>
                <dd>{lastLaunchResult.launchedAtLabel ?? "Not returned"}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Queue posture</dt>
                <dd>{lastLaunchResult.queueLabel ?? "Runtime queue label pending"}</dd>
              </article>
            </div>
            {(lastLaunchResult.warnings?.length ?? 0) > 0 ? (
              <div className="automation-pill-list">
                {lastLaunchResult.warnings?.map((warning) => (
                  <span className="automation-pill" key={warning}>
                    {warning}
                  </span>
                ))}
              </div>
            ) : null}
          </article>
        ) : null}

        {!onLaunch && lastPreparedLaunch?.ready ? (
          <div className="banner banner--warning">
            Launch is prepared, but this view did not expose a launch action. Keep this package
            staged and trigger dispatch from the connected launch surface.
          </div>
        ) : null}

        <div className="inline-actions">
          <button
            className="button"
            type="button"
            disabled={isPreparingLaunch || isLaunching}
            onClick={onPrepareLaunch}
          >
            {isPreparingLaunch ? "Preparing..." : "Prepare"}
          </button>
          <button
            className="button"
            type="button"
            disabled={!canLaunch || isLaunching}
            onClick={onLaunch}
          >
            {isLaunching ? "Launching..." : "Launch"}
          </button>
          <button className="button button--secondary" type="button" onClick={onResetBindings}>
            Reset bindings
          </button>
          <button className="button button--secondary" type="button" onClick={onResetLaunch}>
            Reset launch
          </button>
        </div>
      </div>
    </Panel>
  );
}
