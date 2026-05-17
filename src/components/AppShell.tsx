import type { ReactNode } from "react";

import type { AppRoute } from "../hooks/useHashRoute";
import { NavRail } from "./NavRail";

const TITLES: Record<AppRoute, { title: string; detail: string }> = {
  dashboard: {
    title: "Dashboard",
    detail: "Track local runtime health, queue shape, and Win11 desktop readiness.",
  },
  profiles: {
    title: "Profiles",
    detail: "Core profile workbench with filters, dense tables, and drawer-driven editing.",
  },
  proxies: {
    title: "Proxies",
    detail: "Local proxy center for health, usage, IP region visibility, and profile linkage.",
  },
  automation: {
    title: "Automation",
    detail: "Manage runs, recorder templates, batch dispatch, and local execution workflows.",
  },
  synchronizer: {
    title: "Synchronizer",
    detail: "Control local browser window layout, focus, and multi-window alignment.",
  },
  logs: {
    title: "Logs",
    detail: "Runtime and action log surfaces with paged loading and debounced filtering.",
  },
  settings: {
    title: "Settings",
    detail: "Inspect and control local paths, runtime policies, and desktop defaults.",
  },
};

interface AppShellProps {
  route: AppRoute;
  onNavigate: (route: AppRoute) => void;
  children: ReactNode;
}

export function AppShell({ route, onNavigate, children }: AppShellProps) {
  return (
    <div className="shell">
      <NavRail route={route} onNavigate={onNavigate} />
      <main className="shell__main">
        <header className="shell__header">
          <div>
            <span className="shell__eyebrow">Win11 Local Workspace</span>
            <h1>{TITLES[route].title}</h1>
            <p>{TITLES[route].detail}</p>
          </div>
          <div className="shell__header-chip">Local Fingerprint Browser Workbench</div>
        </header>
        <section className="shell__content">{children}</section>
      </main>
    </div>
  );
}
