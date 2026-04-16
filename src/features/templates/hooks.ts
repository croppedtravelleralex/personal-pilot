import { startTransition, useDeferredValue, useEffect, useMemo } from "react";

import { useDebouncedValue } from "../../hooks/useDebouncedValue";
import { useStore } from "../../store/createStore";
import {
  buildTemplateCompileRequestDraft,
  type TemplateCompileSyncOptions,
} from "./model";
import { templateActions, templatesStore } from "./store";

export function useTemplatesViewModel(syncOptions: TemplateCompileSyncOptions = {}) {
  const state = useStore(templatesStore, (current) => current);
  const debouncedSearch = useDebouncedValue(state.searchInput, 300);
  const deferredSearch = useDeferredValue(debouncedSearch);

  useEffect(() => {
    if (state.appliedSearch !== deferredSearch) {
      startTransition(() => {
        templateActions.applySearch(deferredSearch);
      });
    }
  }, [deferredSearch, state.appliedSearch]);

  useEffect(() => {
    if (state.requestId === 0) {
      void templateActions.refresh();
    }
  }, [state.requestId]);

  const query = state.appliedSearch.trim().toLowerCase();
  const filteredItems = useMemo(
    () =>
      state.items.filter((item) => {
        if (!query) {
          return true;
        }

        const haystack = [
          item.name,
          item.category,
          item.summary,
          item.profileScope,
          item.platformId,
          item.sourceLabel,
          item.readinessLevel,
          ...item.variables.map((variable) => variable.label),
          ...item.steps.map((step) => step.label),
        ]
          .join(" ")
          .toLowerCase();

        return haystack.includes(query);
      }),
    [query, state.items],
  );

  const selectedTemplate =
    state.items.find((item) => item.id === state.selectedTemplateId) ??
    filteredItems[0] ??
    null;

  const selectedBindingDraft = selectedTemplate
    ? state.bindingDrafts[selectedTemplate.id] ?? null
    : null;

  const selectedCompileDraft =
    selectedTemplate && selectedBindingDraft
      ? buildTemplateCompileRequestDraft(
          selectedTemplate,
          selectedBindingDraft,
          syncOptions,
        )
      : null;

  const readyCount = state.items.filter((item) => item.status === "ready").length;

  return {
    state,
    filteredItems,
    selectedTemplate,
    selectedBindingDraft,
    selectedCompileDraft,
    readyCount,
    actions: templateActions,
  };
}
