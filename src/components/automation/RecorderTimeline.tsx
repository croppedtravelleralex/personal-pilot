import { EmptyState } from "../EmptyState";
import {
  InlineContentPreview,
  truncateInlineContent,
} from "../InlineContentPreview";
import { formatRelativeTimestamp } from "../../utils/format";
import type {
  RecorderSessionModel,
  RecorderStepTimelineItem,
} from "../../features/recorder/model";

interface RecorderTimelineProps {
  snapshot: RecorderSessionModel | null;
  selectedStepId: string | null;
  sourceMessage: string;
  isLoading: boolean;
  selectedTemplateId: string | null;
  selectedProfileId: string | null;
  onRefresh: () => void;
  onStart: () => void;
  onPause: () => void;
  onCaptureNext: () => void;
  onStop: () => void;
  onSelectStep: (stepId: string) => void;
}

function getStepBadge(step: RecorderStepTimelineItem): string {
  switch (step.actionType) {
    case "input":
    case "select":
      return step.sensitive ? "badge badge--failed" : "badge badge--warning";
    case "visit":
    case "tab":
      return "badge badge--info";
    default:
      return "badge";
  }
}

export function RecorderTimeline({
  snapshot,
  selectedStepId,
  sourceMessage,
  isLoading,
  selectedTemplateId,
  selectedProfileId,
  onRefresh,
  onStart,
  onPause,
  onCaptureNext,
  onStop,
  onSelectStep,
}: RecorderTimelineProps) {
  const selectedStep =
    snapshot?.steps.find((step) => step.id === selectedStepId) ?? snapshot?.steps[0] ?? null;
  const mismatchWarnings = [
    snapshot && selectedTemplateId && snapshot.templateId && snapshot.templateId !== selectedTemplateId
      ? "Recorder session is bound to a different template than the current launcher selection."
      : null,
    snapshot && selectedProfileId && snapshot.profileId && snapshot.profileId !== selectedProfileId
      ? "Recorder session profile does not match the currently selected run persona."
      : null,
    snapshot && snapshot.source !== "desktop"
      ? "Recorder is currently using adapter-fallback data, so the compile manifest is still useful but queue launch remains a staged operator step."
      : null,
  ].filter((item): item is string => Boolean(item));
  const isRecording = snapshot?.status === "recording";
  const isStopped = snapshot?.status === "stopped";

  return (
    <section className="panel recorder-panel">
      <div className="panel__header">
        <div>
          <span className="shell__eyebrow">Recorder Snapshot</span>
          <h3 className="panel__title">Session Timeline</h3>
          <p className="panel__subtitle">{sourceMessage}</p>
        </div>
        <div className="panel__actions">
          <span
            className={`badge ${
              snapshot?.source === "desktop" ? "badge--info" : "badge--warning"
            }`}
          >
            {snapshot ? `${snapshot.source} / ${snapshot.status}` : "No session"}
          </span>
          <button className="button button--secondary" type="button" onClick={onRefresh}>
            {isLoading ? "Refreshing..." : "Refresh snapshot"}
          </button>
        </div>
      </div>

      {!snapshot ? (
        <EmptyState
          title="No recorder snapshot"
          detail="Select a template to load recorder state from the desktop read model first, with adapter-fallback draft capture only when native capture is unavailable."
        />
      ) : (
        <div className="page-stack">
          <div className="automation-metric-strip automation-metric-strip--compact">
            <article className="automation-metric-strip__item">
              <span className="automation-metric-strip__label">Session</span>
              <strong>{snapshot.sessionId}</strong>
              <small>{snapshot.source}</small>
            </article>
            <article className="automation-metric-strip__item">
              <span className="automation-metric-strip__label">Steps</span>
              <strong>{snapshot.stepCount}</strong>
              <small>{snapshot.variableCount} variable candidates</small>
            </article>
            <article className="automation-metric-strip__item">
              <span className="automation-metric-strip__label">Context</span>
              <strong>{snapshot.profileId ?? "No profile"}</strong>
              <small>{snapshot.platformId ?? "No platform"}</small>
            </article>
          </div>

          {mismatchWarnings.length > 0 ? (
            <div className="banner banner--warning">
              <strong>Recorder context needs review.</strong>
              <div>{mismatchWarnings[0]}</div>
            </div>
          ) : null}

          <div className="recorder-panel__actions">
            <button className="button" type="button" onClick={onStart} disabled={isLoading || isRecording}>
              {isRecording ? "Recording..." : "Start native session"}
            </button>
            <button
              className="button button--secondary"
              type="button"
              onClick={onPause}
              disabled={!snapshot || isStopped}
            >
              Pause
            </button>
            <button
              className="button button--secondary"
              type="button"
              onClick={onCaptureNext}
              disabled={!snapshot}
            >
              Capture preview step
            </button>
            <button
              className="button button--secondary"
              type="button"
              onClick={onStop}
              disabled={!snapshot || isStopped}
            >
              Stop session
            </button>
          </div>

          <div className="recorder-panel__layout">
            <div className="automation-scroll-stack recorder-panel__list">
              <div className="record-list">
                {snapshot.steps.map((step) => {
                  const selected = step.id === selectedStep?.id;

                  return (
                    <article
                      key={step.id}
                      className={[
                        "record-card",
                        "record-card--compact",
                        "record-card--interactive",
                        selected ? "record-card--selected" : "",
                      ]
                        .filter(Boolean)
                        .join(" ")}
                      onClick={() => onSelectStep(step.id)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          onSelectStep(step.id);
                        }
                      }}
                      tabIndex={0}
                    >
                      <div className="record-card__top">
                        <div>
                          <strong>
                            {step.index + 1}. {step.label}
                          </strong>
                          <p className="record-card__subline">{step.tabLabel}</p>
                        </div>
                        <span className={getStepBadge(step)}>{step.actionType}</span>
                      </div>
                      <InlineContentPreview
                        className="record-card__content"
                        value={step.detail}
                        collapseAt={220}
                        expandable={false}
                        copyable={false}
                      />
                      <div className="record-card__footer">
                        <span>{formatRelativeTimestamp(step.capturedAt)}</span>
                        <span>{truncateInlineContent(step.url ?? step.selector ?? "Adapter timeline step", 110)}</span>
                      </div>
                    </article>
                  );
                })}
              </div>
            </div>

            <div className="details-grid details-grid--stacked">
              <div className="details-grid__item">
                <dt>Selected step</dt>
                <dd>
                  {selectedStep ? `${selectedStep.index + 1}. ${selectedStep.label}` : "No step"}
                </dd>
              </div>
              <div className="details-grid__item">
                <dt>Step detail</dt>
                <dd>
                  <InlineContentPreview
                    value={selectedStep?.detail ?? snapshot.note}
                    empty="No recorder detail"
                    collapseAt={240}
                    inlineLimit={8000}
                  />
                </dd>
              </div>
              <div className="details-grid__item">
                <dt>Selector / URL</dt>
                <dd>
                  <InlineContentPreview
                    value={selectedStep?.selector ?? selectedStep?.url}
                    empty="Not captured"
                    collapseAt={160}
                    inlineLimit={6000}
                    mono
                  />
                </dd>
              </div>
              <div className="details-grid__item">
                <dt>Value preview</dt>
                <dd>
                  <InlineContentPreview
                    value={selectedStep?.valuePreview}
                    empty="No value preview"
                    collapseAt={180}
                    inlineLimit={6000}
                    mono={Boolean(selectedStep?.sensitive)}
                  />
                </dd>
              </div>
              <div className="details-grid__item">
                <dt>Current URL</dt>
                <dd>
                  <InlineContentPreview
                    value={snapshot.currentUrl}
                    empty="No captured URL"
                    collapseAt={160}
                    inlineLimit={6000}
                    mono
                  />
                </dd>
              </div>
              <div className="details-grid__item">
                <dt>Variable candidates</dt>
                <dd>{snapshot.variableCandidates.length}</dd>
              </div>
            </div>
          </div>

          {snapshot.variableCandidates.length > 0 ? (
            <div className="automation-pill-list">
              {snapshot.variableCandidates.map((candidate) => (
                <span className="automation-pill" key={`${candidate.key}-${candidate.stepId}`}>
                  {candidate.label}: {candidate.previewValue}
                </span>
              ))}
            </div>
          ) : null}
        </div>
      )}
    </section>
  );
}
