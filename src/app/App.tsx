import { Suspense, lazy } from "react";
import type { ComponentType } from "react";

import { AppShell } from "../components/AppShell";
import type { AppRoute } from "../hooks/useHashRoute";
import { useHashRoute } from "../hooks/useHashRoute";

const DashboardPage = lazy(() =>
  import("../pages/DashboardPage").then((module) => ({
    default: module.DashboardPage,
  })),
);
const ProfilesPage = lazy(() =>
  import("../pages/ProfilesPage").then((module) => ({
    default: module.ProfilesPage,
  })),
);
const ProxiesPage = lazy(() =>
  import("../pages/ProxiesPage").then((module) => ({
    default: module.ProxiesPage,
  })),
);
const AutomationPage = lazy(() =>
  import("../pages/AutomationPage").then((module) => ({
    default: module.AutomationPage,
  })),
);
const SynchronizerPage = lazy(() =>
  import("../pages/SynchronizerPage").then((module) => ({
    default: module.SynchronizerPage,
  })),
);
const LogsPage = lazy(() =>
  import("../pages/LogsPage").then((module) => ({
    default: module.LogsPage,
  })),
);
const SettingsPage = lazy(() =>
  import("../pages/SettingsPage").then((module) => ({
    default: module.SettingsPage,
  })),
);

const ROUTE_COMPONENTS: Record<AppRoute, ComponentType> = {
  dashboard: DashboardPage,
  profiles: ProfilesPage,
  proxies: ProxiesPage,
  automation: AutomationPage,
  synchronizer: SynchronizerPage,
  logs: LogsPage,
  settings: SettingsPage,
};

export function App() {
  const { route, navigate } = useHashRoute();
  const ActivePage = ROUTE_COMPONENTS[route];

  return (
    <AppShell route={route} onNavigate={navigate}>
      <Suspense fallback={null}>
        <ActivePage />
      </Suspense>
    </AppShell>
  );
}
