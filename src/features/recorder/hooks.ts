import { useEffect } from "react";

import { useStore } from "../../store/createStore";
import type { TemplateSummary } from "../templates/model";
import { recorderActions, recorderStore } from "./store";

export function useRecorderViewModel(
  template: TemplateSummary | null,
  context?: { profileId?: string | null },
) {
  const state = useStore(recorderStore, (current) => current);

  useEffect(() => {
    void recorderActions.refresh(template, context);
  }, [context?.profileId, template?.id, template?.platformId]);

  const selectedStep =
    state.snapshot?.steps.find((step) => step.id === state.selectedStepId) ??
    state.snapshot?.steps[0] ??
    null;

  return {
    state,
    selectedStep,
    actions: recorderActions,
  };
}
