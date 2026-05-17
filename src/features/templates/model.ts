import type {
  DesktopTemplateMetadata,
  DesktopTemplateVariableDefinition,
} from "../../types/desktop";
import { formatRelativeTimestamp } from "../../utils/format";

export type TemplateStatus = "draft" | "ready" | "placeholder" | "disabled" | string;
export type TemplateVariableKind =
  | "text"
  | "url"
  | "number"
  | "duration"
  | "textarea"
  | "tag_bundle";
export type TemplateCatalogSource = "desktop" | "seed" | "adapter_fallback";
export type TemplateBindingValueSource =
  | "empty"
  | "default"
  | "draft"
  | "recorder"
  | "run_context";
export type TemplateBindingStatus = "missing" | "ready" | "optional";
export type TemplateCompileTargetSource = "bindings" | "selected_run" | "unbound";
export type TemplateCompileRecorderSource = "desktop" | "adapter_fallback" | "none";

export interface TemplateVariable {
  key: string;
  label: string;
  required: boolean;
  sensitive: boolean;
  example: string;
  defaultValue: string;
  kind: TemplateVariableKind;
  source: string;
}

export interface TemplateStepOutline {
  id: string;
  index: number;
  actionType: string;
  label: string;
  detail: string;
  variableKeys: string[];
  sensitive: boolean;
  tabLabel: string;
  waitMs: number | null;
}

export interface TemplateSummary {
  id: string;
  name: string;
  category: string;
  status: TemplateStatus;
  platformId: string;
  storeId: string | null;
  sourceLabel: string;
  readinessLevel: string;
  dataSource: TemplateCatalogSource;
  stepCount: number;
  variableCount: number;
  profileScope: string;
  updatedAt: string;
  updatedLabel: string;
  summary: string;
  compilerState: string;
  coverageLabel: string;
  allowedRegions: string[];
  variables: TemplateVariable[];
  steps: TemplateStepOutline[];
}

export interface TemplateBindingValueDraft {
  key: string;
  label: string;
  value: string;
  required: boolean;
  sensitive: boolean;
  example: string;
  source: TemplateBindingValueSource;
  status: TemplateBindingStatus;
  error: string | null;
}

export interface TemplateBindingDraft {
  templateId: string;
  values: Record<string, TemplateBindingValueDraft>;
  note: string;
  profileIdsInput: string;
  profileIds: string[];
  completionCount: number;
  missingRequiredCount: number;
  updatedAt: string | null;
}

export interface TemplateCompileRequestBinding {
  key: string;
  value: string;
  required: boolean;
  sensitive: boolean;
  source: TemplateBindingValueSource;
}

export interface TemplateCompileRequestDraft {
  templateId: string;
  templateName: string;
  platformId: string;
  targetProfileIds: string[];
  targetSource: TemplateCompileTargetSource;
  bindings: TemplateCompileRequestBinding[];
  stepIds: string[];
  stepCount: number;
  missingRequiredKeys: string[];
  note: string;
  recorderSessionId: string | null;
  recorderSource: TemplateCompileRecorderSource;
  recorderStepCount: number;
  warnings: string[];
  ready: boolean;
  generatedAt: string;
}

export interface TemplateCompileSyncOptions {
  selectedRunProfileId?: string | null;
  recorderSessionId?: string | null;
  recorderStepCount?: number;
  recorderSource?: TemplateCompileRecorderSource | null;
}

function toEpochSeconds(value: string): string {
  return String(Math.floor(new Date(value).getTime() / 1000));
}

function inferVariableKind(
  variableKey: string,
  example: string,
  source: string,
): TemplateVariableKind {
  const normalizedKey = variableKey.toLowerCase();
  const normalizedExample = example.toLowerCase();
  const normalizedSource = source.toLowerCase();

  if (normalizedKey.includes("url") || normalizedExample.startsWith("http")) {
    return "url";
  }

  if (
    normalizedKey.includes("seconds") ||
    normalizedKey.includes("wait") ||
    normalizedKey.includes("delay")
  ) {
    return "duration";
  }

  if (normalizedKey.includes("count") || normalizedKey.includes("limit")) {
    return "number";
  }

  if (
    normalizedKey.includes("body") ||
    normalizedKey.includes("note") ||
    normalizedKey.includes("text") ||
    normalizedSource.includes("textarea")
  ) {
    return "textarea";
  }

  if (normalizedKey.includes("tag")) {
    return "tag_bundle";
  }

  return "text";
}

function makeVariable(
  definition: Omit<TemplateVariable, "kind"> & { kind?: TemplateVariableKind },
): TemplateVariable {
  return {
    ...definition,
    kind:
      definition.kind ??
      inferVariableKind(definition.key, definition.example, definition.source),
  };
}

const SEED_TEMPLATE_CATALOG: TemplateSummary[] = [
  {
    id: "welcome-pass",
    name: "Welcome Pass",
    category: "Profile Warmup",
    status: "ready",
    platformId: "generic-browser",
    storeId: "starter-store",
    sourceLabel: "Recorder seed",
    readinessLevel: "ready",
    dataSource: "seed",
    stepCount: 6,
    variableCount: 2,
    profileScope: "Starter profile group",
    updatedAt: toEpochSeconds("2026-04-15T21:02:00+08:00"),
    updatedLabel: formatRelativeTimestamp(
      toEpochSeconds("2026-04-15T21:02:00+08:00"),
    ),
    summary:
      "Open the entry page, settle on the first stable tab, and capture a reusable warmup path for later queue execution.",
    compilerState: "Binding-ready adapter draft",
    coverageLabel: "Warm / revisit coverage",
    allowedRegions: ["US", "GB"],
    variables: [
      makeVariable({
        key: "entry_url",
        label: "Entry URL",
        required: true,
        sensitive: false,
        example: "https://example.com/welcome",
        defaultValue: "https://example.com/welcome",
        source: "template_default",
      }),
      makeVariable({
        key: "linger_seconds",
        label: "Linger Seconds",
        required: false,
        sensitive: false,
        example: "18",
        defaultValue: "18",
        source: "template_default",
      }),
    ],
    steps: [
      {
        id: "welcome-step-1",
        index: 0,
        actionType: "visit",
        label: "Open entry page",
        detail: "Visit the bound entry URL and wait for the first stable document load.",
        variableKeys: ["entry_url"],
        sensitive: false,
        tabLabel: "Entry",
        waitMs: 0,
      },
      {
        id: "welcome-step-2",
        index: 1,
        actionType: "wait",
        label: "Hold on first viewport",
        detail: "Stay on the first viewport long enough to establish the warmup baseline.",
        variableKeys: ["linger_seconds"],
        sensitive: false,
        tabLabel: "Entry",
        waitMs: 18000,
      },
      {
        id: "welcome-step-3",
        index: 2,
        actionType: "scroll",
        label: "Scroll hero section",
        detail: "Scroll a short distance to emulate first-page engagement.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Entry",
        waitMs: 800,
      },
      {
        id: "welcome-step-4",
        index: 3,
        actionType: "click",
        label: "Open first content tile",
        detail: "Follow the first stable content tile if present.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Entry",
        waitMs: 900,
      },
      {
        id: "welcome-step-5",
        index: 4,
        actionType: "wait",
        label: "Allow landing page settle",
        detail: "Wait until the landing page is stable for the final URL snapshot.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Landing",
        waitMs: 2400,
      },
      {
        id: "welcome-step-6",
        index: 5,
        actionType: "tab",
        label: "Record final tab context",
        detail: "Capture the final tab title and URL as execution output context.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Landing",
        waitMs: null,
      },
    ],
  },
  {
    id: "comment-followup",
    name: "Comment Follow-up",
    category: "Engagement",
    status: "draft",
    platformId: "social-thread",
    storeId: "engagement-store",
    sourceLabel: "Recorder draft",
    readinessLevel: "sample_ready",
    dataSource: "seed",
    stepCount: 8,
    variableCount: 3,
    profileScope: "Tagged reply cohort",
    updatedAt: toEpochSeconds("2026-04-15T18:18:00+08:00"),
    updatedLabel: formatRelativeTimestamp(
      toEpochSeconds("2026-04-15T18:18:00+08:00"),
    ),
    summary:
      "Replay a browse path, focus the comment region, and prepare a reusable text handoff without persisting sensitive form state.",
    compilerState: "Waiting for deeper recorder capture",
    coverageLabel: "Stateful reply path",
    allowedRegions: ["US"],
    variables: [
      makeVariable({
        key: "post_url",
        label: "Post URL",
        required: true,
        sensitive: false,
        example: "https://example.com/post/42",
        defaultValue: "",
        source: "recorder_variable",
      }),
      makeVariable({
        key: "reply_text",
        label: "Reply Text",
        required: true,
        sensitive: false,
        example: "Thanks, following up on this thread.",
        defaultValue: "",
        source: "recorder_variable",
      }),
      makeVariable({
        key: "wait_after_submit",
        label: "Wait After Submit",
        required: false,
        sensitive: false,
        example: "5",
        defaultValue: "5",
        source: "template_default",
      }),
    ],
    steps: [
      {
        id: "comment-step-1",
        index: 0,
        actionType: "visit",
        label: "Open target post",
        detail: "Visit the target thread before entering the reply flow.",
        variableKeys: ["post_url"],
        sensitive: false,
        tabLabel: "Post",
        waitMs: 0,
      },
      {
        id: "comment-step-2",
        index: 1,
        actionType: "wait",
        label: "Wait for comments region",
        detail: "Pause until the comments region becomes stable.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Post",
        waitMs: 1500,
      },
      {
        id: "comment-step-3",
        index: 2,
        actionType: "click",
        label: "Focus reply editor",
        detail: "Click the reply entry field or CTA.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Post",
        waitMs: 600,
      },
      {
        id: "comment-step-4",
        index: 3,
        actionType: "input",
        label: "Type reply text",
        detail: "Inject the bound reply text into the editor.",
        variableKeys: ["reply_text"],
        sensitive: false,
        tabLabel: "Post",
        waitMs: null,
      },
      {
        id: "comment-step-5",
        index: 4,
        actionType: "click",
        label: "Submit reply",
        detail: "Click the submit button once text binding is ready.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Post",
        waitMs: 400,
      },
      {
        id: "comment-step-6",
        index: 5,
        actionType: "wait",
        label: "Post-submit hold",
        detail: "Allow the UI to settle after submission.",
        variableKeys: ["wait_after_submit"],
        sensitive: false,
        tabLabel: "Post",
        waitMs: 5000,
      },
      {
        id: "comment-step-7",
        index: 6,
        actionType: "scroll",
        label: "Capture reply position",
        detail: "Scroll slightly to confirm the new reply is present.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Post",
        waitMs: 500,
      },
      {
        id: "comment-step-8",
        index: 7,
        actionType: "tab",
        label: "Persist post-run tab snapshot",
        detail: "Keep the final tab context ready for run detail review.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Post",
        waitMs: null,
      },
    ],
  },
  {
    id: "window-health-check",
    name: "Window Health Check",
    category: "Diagnostics",
    status: "ready",
    platformId: "runtime-monitor",
    storeId: null,
    sourceLabel: "Platform template seed",
    readinessLevel: "ready",
    dataSource: "seed",
    stepCount: 4,
    variableCount: 1,
    profileScope: "All active local profiles",
    updatedAt: toEpochSeconds("2026-04-15T22:10:00+08:00"),
    updatedLabel: formatRelativeTimestamp(
      toEpochSeconds("2026-04-15T22:10:00+08:00"),
    ),
    summary:
      "Run a lightweight diagnostic pass, confirm window availability, and keep the launcher chain ready for local runtime dispatch.",
    compilerState: "Launch draft ready",
    coverageLabel: "Diagnostic path",
    allowedRegions: [],
    variables: [
      makeVariable({
        key: "target_surface",
        label: "Target Surface",
        required: true,
        sensitive: false,
        example: "dashboard",
        defaultValue: "dashboard",
        source: "template_default",
      }),
    ],
    steps: [
      {
        id: "health-step-1",
        index: 0,
        actionType: "tab",
        label: "Capture current surface",
        detail: "Remember the current browser surface before diagnostics begin.",
        variableKeys: ["target_surface"],
        sensitive: false,
        tabLabel: "Main",
        waitMs: null,
      },
      {
        id: "health-step-2",
        index: 1,
        actionType: "wait",
        label: "Poll runtime state",
        detail: "Pause briefly while runtime status is checked.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Main",
        waitMs: 1200,
      },
      {
        id: "health-step-3",
        index: 2,
        actionType: "click",
        label: "Open health surface",
        detail: "Navigate to the requested surface for a quick health pass.",
        variableKeys: ["target_surface"],
        sensitive: false,
        tabLabel: "Main",
        waitMs: 700,
      },
      {
        id: "health-step-4",
        index: 3,
        actionType: "tab",
        label: "Record health snapshot",
        detail: "Capture the final state for run detail inspection.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Diagnostics",
        waitMs: null,
      },
    ],
  },
  {
    id: "sync-post-pass",
    name: "Sync Post Pass",
    category: "Publishing",
    status: "placeholder",
    platformId: "content-publish",
    storeId: "shared-publish",
    sourceLabel: "Future compile adapter",
    readinessLevel: "baseline",
    dataSource: "seed",
    stepCount: 7,
    variableCount: 4,
    profileScope: "Awaiting profile binder",
    updatedAt: toEpochSeconds("2026-04-15T16:46:00+08:00"),
    updatedLabel: formatRelativeTimestamp(
      toEpochSeconds("2026-04-15T16:46:00+08:00"),
    ),
    summary:
      "Reserve launch structure for a publish flow while compile and queue contracts are still missing.",
    compilerState: "Compile contract blocked",
    coverageLabel: "Write path placeholder",
    allowedRegions: ["CN", "US"],
    variables: [
      makeVariable({
        key: "draft_title",
        label: "Draft Title",
        required: true,
        sensitive: false,
        example: "Today automation snapshot",
        defaultValue: "",
        source: "recorder_variable",
      }),
      makeVariable({
        key: "draft_body",
        label: "Draft Body",
        required: true,
        sensitive: false,
        example: "A short structured note.",
        defaultValue: "",
        source: "recorder_variable",
      }),
      makeVariable({
        key: "tag_bundle",
        label: "Tag Bundle",
        required: false,
        sensitive: false,
        example: "alpha,beta",
        defaultValue: "alpha,beta",
        source: "template_default",
      }),
      makeVariable({
        key: "publish_window",
        label: "Publish Window",
        required: false,
        sensitive: false,
        example: "09:00-11:00",
        defaultValue: "",
        source: "template_default",
      }),
    ],
    steps: [
      {
        id: "publish-step-1",
        index: 0,
        actionType: "visit",
        label: "Open draft console",
        detail: "Enter the draft console before content input begins.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Draft",
        waitMs: 0,
      },
      {
        id: "publish-step-2",
        index: 1,
        actionType: "input",
        label: "Bind draft title",
        detail: "Populate the title field from the binding draft.",
        variableKeys: ["draft_title"],
        sensitive: false,
        tabLabel: "Draft",
        waitMs: null,
      },
      {
        id: "publish-step-3",
        index: 2,
        actionType: "input",
        label: "Bind draft body",
        detail: "Populate the main body field from bindings.",
        variableKeys: ["draft_body"],
        sensitive: false,
        tabLabel: "Draft",
        waitMs: null,
      },
      {
        id: "publish-step-4",
        index: 3,
        actionType: "input",
        label: "Apply tags",
        detail: "Insert the optional tag bundle.",
        variableKeys: ["tag_bundle"],
        sensitive: false,
        tabLabel: "Draft",
        waitMs: null,
      },
      {
        id: "publish-step-5",
        index: 4,
        actionType: "wait",
        label: "Hold until publish window",
        detail: "Keep placeholder wait logic for later queue integration.",
        variableKeys: ["publish_window"],
        sensitive: false,
        tabLabel: "Draft",
        waitMs: 1200,
      },
      {
        id: "publish-step-6",
        index: 5,
        actionType: "click",
        label: "Queue publish action",
        detail: "This remains a launch placeholder until write commands land.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Draft",
        waitMs: 600,
      },
      {
        id: "publish-step-7",
        index: 6,
        actionType: "tab",
        label: "Store publish context",
        detail: "Capture final tab context for run detail.",
        variableKeys: [],
        sensitive: false,
        tabLabel: "Draft",
        waitMs: null,
      },
    ],
  },
];

function mapVariableDefinition(
  definition: DesktopTemplateVariableDefinition,
): TemplateVariable {
  const defaultValue =
    typeof definition.defaultValue === "string"
      ? definition.defaultValue
      : definition.defaultValue === null
        ? ""
        : JSON.stringify(definition.defaultValue);

  return makeVariable({
    key: definition.key,
    label: definition.label ?? definition.key,
    required: definition.required,
    sensitive: definition.sensitive,
    example: defaultValue || definition.key,
    defaultValue,
    source: definition.source,
  });
}

function createDesktopFallbackSteps(
  metadata: DesktopTemplateMetadata,
): TemplateStepOutline[] {
  const total = Math.max(1, Math.min(metadata.coverage.stepCount || 1, 6));

  return Array.from({ length: total }, (_, index) => ({
    id: `${metadata.id}-desktop-step-${index + 1}`,
    index,
    actionType: index === 0 ? "visit" : index === total - 1 ? "tab" : "wait",
    label: index === 0 ? "Open template entry" : `Desktop outline step ${index + 1}`,
    detail:
      "Desktop metadata is available, but detailed template step read models still rely on the feature adapter.",
    variableKeys:
      index === 0 && metadata.variableDefinitions[0]
        ? [metadata.variableDefinitions[0].key]
        : [],
    sensitive: false,
    tabLabel: index === total - 1 ? "Output" : "Flow",
    waitMs: index === 0 ? 0 : 1200,
  }));
}

function deriveTemplateStatus(metadata: DesktopTemplateMetadata): TemplateStatus {
  if (metadata.status === "disabled") {
    return "disabled";
  }

  if (metadata.readinessLevel === "ready" || metadata.readinessLevel === "sample_ready") {
    return "ready";
  }

  if (metadata.readinessLevel === "baseline") {
    return "placeholder";
  }

  return "draft";
}

export function createSeedTemplates(): TemplateSummary[] {
  return SEED_TEMPLATE_CATALOG.map((template) => ({
    ...template,
    allowedRegions: [...template.allowedRegions],
    variables: template.variables.map((variable) => ({
      ...variable,
    })),
    steps: template.steps.map((step) => ({
      ...step,
      variableKeys: [...step.variableKeys],
    })),
  }));
}

export function mapDesktopTemplateMetadata(
  metadata: DesktopTemplateMetadata,
  seedTemplate?: TemplateSummary,
): TemplateSummary {
  const variables = metadata.variableDefinitions.map(mapVariableDefinition);
  const steps = seedTemplate?.steps.length
    ? seedTemplate.steps.map((step) => ({
        ...step,
        variableKeys: [...step.variableKeys],
      }))
    : createDesktopFallbackSteps(metadata);

  return {
    id: metadata.id,
    name: metadata.name,
    category: seedTemplate?.category ?? metadata.platformId,
    status: deriveTemplateStatus(metadata),
    platformId: metadata.platformId,
    storeId: metadata.storeId,
    sourceLabel: metadata.source,
    readinessLevel: metadata.readinessLevel,
    dataSource: "desktop",
    stepCount: metadata.coverage.stepCount,
    variableCount: variables.length,
    profileScope:
      seedTemplate?.profileScope ??
      (metadata.storeId ? `${metadata.platformId} / ${metadata.storeId}` : metadata.platformId),
    updatedAt: metadata.updatedAt,
    updatedLabel: formatRelativeTimestamp(metadata.updatedAt),
    summary:
      seedTemplate?.summary ??
      `Desktop metadata loaded for ${metadata.platformId}. Detailed recorder-backed flow still comes from the feature adapter.`,
    compilerState:
      metadata.readinessLevel === "ready"
        ? "Metadata ready, compile contract pending"
        : `Readiness ${metadata.readinessLevel}`,
    coverageLabel:
      seedTemplate?.coverageLabel ??
      `${metadata.coverage.stepCount} steps / ${metadata.coverage.variableCount} vars`,
    allowedRegions: [...metadata.allowedRegions],
    variables,
    steps,
  };
}

export function createTemplateBindingDraft(
  template: TemplateSummary,
): TemplateBindingDraft {
  const values = Object.fromEntries(
    template.variables.map((variable) => {
      const initialValue = variable.defaultValue.trim();
      const isMissing = variable.required && initialValue.length === 0;

      return [
        variable.key,
        {
          key: variable.key,
          label: variable.label,
          value: initialValue,
          required: variable.required,
          sensitive: variable.sensitive,
          example: variable.example,
          source: initialValue ? "default" : "empty",
          status: isMissing ? "missing" : variable.required ? "ready" : "optional",
          error: isMissing ? `${variable.label} is required` : null,
        } satisfies TemplateBindingValueDraft,
      ];
    }),
  );

  const completionCount = Object.values(values).filter(
    (value) => !value.required || value.value.trim().length > 0,
  ).length;
  const missingRequiredCount = Object.values(values).filter(
    (value) => value.required && value.value.trim().length === 0,
  ).length;

  return {
    templateId: template.id,
    values,
    note: "",
    profileIdsInput: "",
    profileIds: [],
    completionCount,
    missingRequiredCount,
    updatedAt: null,
  };
}

export function updateTemplateBindingDraft(
  draft: TemplateBindingDraft,
  variableKey: string,
  value: string,
  source: TemplateBindingValueSource = "draft",
): TemplateBindingDraft {
  const current = draft.values[variableKey];
  if (!current) {
    return draft;
  }

  const trimmedValue = value.trim();
  const nextValue: TemplateBindingValueDraft = {
    ...current,
    value,
    source,
    status:
      current.required && trimmedValue.length === 0
        ? "missing"
        : current.required
          ? "ready"
          : "optional",
    error:
      current.required && trimmedValue.length === 0 ? `${current.label} is required` : null,
  };

  const values = {
    ...draft.values,
    [variableKey]: nextValue,
  };

  return {
    ...draft,
    values,
    completionCount: Object.values(values).filter(
      (item) => !item.required || item.value.trim().length > 0,
    ).length,
    missingRequiredCount: Object.values(values).filter(
      (item) => item.required && item.value.trim().length === 0,
    ).length,
    updatedAt: String(Math.floor(Date.now() / 1000)),
  };
}

export function updateTemplateBindingProfileIds(
  draft: TemplateBindingDraft,
  profileIdsInput: string,
): TemplateBindingDraft {
  const profileIds = profileIdsInput
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);

  return {
    ...draft,
    profileIdsInput,
    profileIds,
    updatedAt: String(Math.floor(Date.now() / 1000)),
  };
}

export function updateTemplateBindingNote(
  draft: TemplateBindingDraft,
  note: string,
): TemplateBindingDraft {
  return {
    ...draft,
    note,
    updatedAt: String(Math.floor(Date.now() / 1000)),
  };
}

export function buildTemplateCompileRequestDraft(
  template: TemplateSummary,
  bindingDraft: TemplateBindingDraft,
  syncOptions: TemplateCompileSyncOptions = {},
): TemplateCompileRequestDraft {
  const selectedRunProfileId = syncOptions.selectedRunProfileId?.trim();
  const targetSource: TemplateCompileTargetSource =
    bindingDraft.profileIds.length > 0
      ? "bindings"
      : selectedRunProfileId
        ? "selected_run"
        : "unbound";
  const targetProfileIds =
    targetSource === "bindings"
      ? [...bindingDraft.profileIds]
      : targetSource === "selected_run" && selectedRunProfileId
        ? [selectedRunProfileId]
        : [];
  const recorderSource = syncOptions.recorderSource ?? "none";

  const bindings = Object.values(bindingDraft.values).map((value) => ({
    key: value.key,
    value: value.value,
    required: value.required,
    sensitive: value.sensitive,
    source: value.source,
  }));

  const missingRequiredKeys = bindings
    .filter((binding) => binding.required && binding.value.trim().length === 0)
    .map((binding) => binding.key);

  const warnings: string[] = [];
  if (targetProfileIds.length === 0) {
    warnings.push("No explicit target profile ids are bound yet.");
  } else if (targetSource === "selected_run") {
    warnings.push("Target profile currently falls back to the selected run persona.");
  }
  if (recorderSource !== "desktop") {
    warnings.push(
      recorderSource === "adapter_fallback"
        ? "Recorder session is using adapter fallback data instead of a native capture session."
        : "Recorder session is not attached yet.",
    );
  }
  if ((syncOptions.recorderStepCount ?? 0) === 0) {
    warnings.push("Recorder timeline is empty, compile preview only includes template outline.");
  }
  if (template.status !== "ready") {
    warnings.push(`Template readiness is still ${template.status}.`);
  }

  return {
    templateId: template.id,
    templateName: template.name,
    platformId: template.platformId,
    targetProfileIds,
    targetSource,
    bindings,
    stepIds: template.steps.map((step) => step.id),
    stepCount: template.steps.length,
    missingRequiredKeys,
    note: bindingDraft.note,
    recorderSessionId: syncOptions.recorderSessionId ?? null,
    recorderSource,
    recorderStepCount: syncOptions.recorderStepCount ?? 0,
    warnings,
    ready: missingRequiredKeys.length === 0 && targetProfileIds.length > 0,
    generatedAt: String(Math.floor(Date.now() / 1000)),
  };
}
