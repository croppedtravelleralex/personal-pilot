import * as desktop from "../../services/desktop";
import { createStore } from "../../store/createStore";
import type { DesktopServiceError } from "../../services/desktop";
import type { DesktopTemplateMetadataPage } from "../../types/desktop";
import {
  createSeedTemplates,
  createTemplateBindingDraft,
  mapDesktopTemplateMetadata,
  updateTemplateBindingDraft,
  updateTemplateBindingNote,
  updateTemplateBindingProfileIds,
  type TemplateBindingDraft,
  type TemplateCatalogSource,
  type TemplateSummary,
} from "./model";

interface TemplatesState {
  items: TemplateSummary[];
  selectedTemplateId: string | null;
  searchInput: string;
  appliedSearch: string;
  bindingDrafts: Record<string, TemplateBindingDraft>;
  isLoading: boolean;
  error: string | null;
  requestId: number;
  source: TemplateCatalogSource;
  sourceMessage: string;
}

function buildBindingDrafts(items: TemplateSummary[]): Record<string, TemplateBindingDraft> {
  return Object.fromEntries(
    items.map((item) => [item.id, createTemplateBindingDraft(item)]),
  );
}

const SEED_ITEMS = createSeedTemplates();

const templatesStore = createStore<TemplatesState>({
  items: SEED_ITEMS,
  selectedTemplateId: SEED_ITEMS[0]?.id ?? null,
  searchInput: "",
  appliedSearch: "",
  bindingDrafts: buildBindingDrafts(SEED_ITEMS),
  isLoading: false,
  error: null,
  requestId: 0,
  source: "seed",
  sourceMessage:
    "Template catalog starts from seed rows and upgrades to the desktop read model when it responds.",
});

function isCommandNotReady(error: unknown): error is DesktopServiceError {
  return (
    Boolean(error) &&
    typeof error === "object" &&
    "code" in error &&
    (error as { code?: string }).code === "desktop_command_not_ready"
  );
}

function mergeTemplateCatalog(page: DesktopTemplateMetadataPage): TemplateSummary[] {
  const seedMap = new Map(SEED_ITEMS.map((item) => [item.id, item]));
  const mappedDesktop = page.items.map((item) =>
    mapDesktopTemplateMetadata(item, seedMap.get(item.id)),
  );
  const merged = [...mappedDesktop];
  const existingIds = new Set(mappedDesktop.map((item) => item.id));

  for (const seed of SEED_ITEMS) {
    if (!existingIds.has(seed.id)) {
      merged.push({
        ...seed,
        dataSource: "adapter_fallback",
      });
    }
  }

  return merged;
}

function preserveBindingDrafts(
  items: TemplateSummary[],
  existingDrafts: Record<string, TemplateBindingDraft>,
): Record<string, TemplateBindingDraft> {
  return Object.fromEntries(
    items.map((item) => {
      const existing = existingDrafts[item.id];
      if (!existing) {
        return [item.id, createTemplateBindingDraft(item)];
      }

      const seedDraft = createTemplateBindingDraft(item);
      const values = Object.fromEntries(
        Object.entries(seedDraft.values).map(([key, value]) => [
          key,
          existing.values[key] ?? value,
        ]),
      );

      return [
        item.id,
        {
          ...existing,
          values,
          completionCount: Object.values(values).filter(
            (draftValue) =>
              !draftValue.required || draftValue.value.trim().length > 0,
          ).length,
          missingRequiredCount: Object.values(values).filter(
            (draftValue) =>
              draftValue.required && draftValue.value.trim().length === 0,
          ).length,
        },
      ];
    }),
  );
}

function ensureSelectedTemplateId(
  items: TemplateSummary[],
  currentSelectedId: string | null,
): string | null {
  if (currentSelectedId && items.some((item) => item.id === currentSelectedId)) {
    return currentSelectedId;
  }

  return items[0]?.id ?? null;
}

export const templateActions = {
  setSearchInput(searchInput: string) {
    templatesStore.setState((current) => ({
      ...current,
      searchInput,
    }));
  },
  applySearch(appliedSearch: string) {
    templatesStore.setState((current) => {
      if (current.appliedSearch === appliedSearch) {
        return current;
      }

      return {
        ...current,
        appliedSearch,
      };
    });
  },
  selectTemplate(templateId: string) {
    templatesStore.setState((current) => {
      if (!current.items.some((item) => item.id === templateId)) {
        return current;
      }

      return {
        ...current,
        selectedTemplateId: templateId,
      };
    });
  },
  setBindingValue(templateId: string, variableKey: string, value: string) {
    templatesStore.setState((current) => {
      const draft = current.bindingDrafts[templateId];
      if (!draft) {
        return current;
      }

      return {
        ...current,
        bindingDrafts: {
          ...current.bindingDrafts,
          [templateId]: updateTemplateBindingDraft(draft, variableKey, value),
        },
      };
    });
  },
  hydrateBindingValueFromRecorder(templateId: string, variableKey: string, value: string) {
    templatesStore.setState((current) => {
      const draft = current.bindingDrafts[templateId];
      if (!draft) {
        return current;
      }

      return {
        ...current,
        bindingDrafts: {
          ...current.bindingDrafts,
          [templateId]: updateTemplateBindingDraft(draft, variableKey, value, "recorder"),
        },
      };
    });
  },
  hydrateBindingValueFromRunContext(templateId: string, variableKey: string, value: string) {
    templatesStore.setState((current) => {
      const draft = current.bindingDrafts[templateId];
      if (!draft) {
        return current;
      }

      return {
        ...current,
        bindingDrafts: {
          ...current.bindingDrafts,
          [templateId]: updateTemplateBindingDraft(draft, variableKey, value, "run_context"),
        },
      };
    });
  },
  setBindingNote(templateId: string, note: string) {
    templatesStore.setState((current) => {
      const draft = current.bindingDrafts[templateId];
      if (!draft) {
        return current;
      }

      return {
        ...current,
        bindingDrafts: {
          ...current.bindingDrafts,
          [templateId]: updateTemplateBindingNote(draft, note),
        },
      };
    });
  },
  setBindingProfileIdsInput(templateId: string, profileIdsInput: string) {
    templatesStore.setState((current) => {
      const draft = current.bindingDrafts[templateId];
      if (!draft) {
        return current;
      }

      return {
        ...current,
        bindingDrafts: {
          ...current.bindingDrafts,
          [templateId]: updateTemplateBindingProfileIds(draft, profileIdsInput),
        },
      };
    });
  },
  resetBindingDraft(templateId: string) {
    templatesStore.setState((current) => {
      const template = current.items.find((item) => item.id === templateId);
      if (!template) {
        return current;
      }

      return {
        ...current,
        bindingDrafts: {
          ...current.bindingDrafts,
          [templateId]: createTemplateBindingDraft(template),
        },
      };
    });
  },
  async refresh() {
    const snapshot = templatesStore.getState();
    const requestId = snapshot.requestId + 1;

    templatesStore.setState((current) => ({
      ...current,
      isLoading: true,
      error: null,
      requestId,
    }));

    try {
      const page = await desktop.listTemplateMetadataPage({
        page: 1,
        pageSize: 50,
      });

      if (templatesStore.getState().requestId !== requestId) {
        return;
      }

      const items = mergeTemplateCatalog(page);

      templatesStore.setState((current) => ({
        ...current,
        items,
        selectedTemplateId: ensureSelectedTemplateId(items, current.selectedTemplateId),
        bindingDrafts: preserveBindingDrafts(items, current.bindingDrafts),
        isLoading: false,
        error: null,
        source: "desktop",
        sourceMessage:
          "Desktop template metadata is primary. Adapter rows are used only when a template is absent from the desktop catalog.",
      }));
    } catch (error) {
      if (templatesStore.getState().requestId !== requestId) {
        return;
      }

      const commandNotReady = isCommandNotReady(error);
      const normalizedMessage =
        error instanceof Error ? error.message : "Failed to load template metadata";

      templatesStore.setState((current) => ({
        ...current,
        isLoading: false,
        error: commandNotReady ? null : normalizedMessage,
        source: commandNotReady ? "adapter_fallback" : current.source,
        sourceMessage: commandNotReady
          ? "This desktop build does not expose template metadata yet. The adapter catalog stays available."
          : "Desktop template read failed. Keeping the last successful catalog without switching away from native-first mode.",
      }));
    }
  },
};

export { templatesStore };
