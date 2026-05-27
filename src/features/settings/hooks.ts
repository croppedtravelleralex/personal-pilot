import { useEffect } from "react";

import { useStore } from "../../store/createStore";
import {
  areBrowserEnvironmentPolicyDraftEqual,
  areCamoufoxSettingsDraftEqual,
  areLocalApiSettingsDraftEqual,
  areSettingsDraftEqual,
  DEFAULT_BROWSER_ENVIRONMENT_POLICY_DRAFT,
  DEFAULT_CAMOUFOX_SETTINGS_DRAFT,
  DEFAULT_LOCAL_API_SETTINGS_DRAFT,
  DEFAULT_RUNTIME_SETTINGS_DRAFT,
  settingsActions,
  settingsStore,
} from "./store";

export function useSettingsViewModel() {
  const state = useStore(settingsStore, (current) => current);

  useEffect(() => {
    if (!state.snapshot && !state.isLoading) {
      void settingsActions.refresh();
    }
  }, [state.isLoading, state.snapshot]);

  const baselineDraft = state.loadedDraft ?? DEFAULT_RUNTIME_SETTINGS_DRAFT;
  const localApiBaselineDraft =
    state.loadedLocalApiDraft ?? DEFAULT_LOCAL_API_SETTINGS_DRAFT;
  const browserEnvironmentBaselineDraft =
    state.loadedBrowserEnvironmentDraft ?? DEFAULT_BROWSER_ENVIRONMENT_POLICY_DRAFT;
  const camoufoxBaselineDraft =
    state.loadedCamoufoxDraft ?? DEFAULT_CAMOUFOX_SETTINGS_DRAFT;
  const runtimeIsDirty = !areSettingsDraftEqual(state.draft, baselineDraft);
  const localApiIsDirty = !areLocalApiSettingsDraftEqual(
    state.localApiDraft,
    localApiBaselineDraft,
  );
  const browserEnvironmentIsDirty = !areBrowserEnvironmentPolicyDraftEqual(
    state.browserEnvironmentDraft,
    browserEnvironmentBaselineDraft,
  );
  const camoufoxIsDirty = !areCamoufoxSettingsDraftEqual(
    state.camoufoxDraft,
    camoufoxBaselineDraft,
  );

  return {
    state,
    isDirty:
      runtimeIsDirty ||
      localApiIsDirty ||
      browserEnvironmentIsDirty ||
      camoufoxIsDirty,
    runtimeIsDirty,
    localApiIsDirty,
    browserEnvironmentIsDirty,
    camoufoxIsDirty,
    defaultsDraft: DEFAULT_RUNTIME_SETTINGS_DRAFT,
    localApiDefaultsDraft: DEFAULT_LOCAL_API_SETTINGS_DRAFT,
    browserEnvironmentDefaultsDraft: DEFAULT_BROWSER_ENVIRONMENT_POLICY_DRAFT,
    camoufoxDefaultsDraft: DEFAULT_CAMOUFOX_SETTINGS_DRAFT,
    actions: settingsActions,
  };
}
