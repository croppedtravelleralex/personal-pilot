import type { AppRoute } from "../hooks/useHashRoute";

const ITEMS: Array<{ route: AppRoute; label: string; detail: string }> = [
  { route: "dashboard", label: "Dashboard", detail: "Runtime and KPIs" },
  { route: "profiles", label: "Profiles", detail: "Core workbench" },
  { route: "proxies", label: "Proxies", detail: "Proxy center" },
  { route: "automation", label: "Automation", detail: "Runs, tasks, and recorder" },
  { route: "synchronizer", label: "Synchronizer", detail: "Window matrix" },
  { route: "logs", label: "Logs", detail: "Runtime and actions" },
  { route: "settings", label: "Settings", detail: "Local control" },
];

interface NavRailProps {
  route: AppRoute;
  onNavigate: (route: AppRoute) => void;
}

export function NavRail({ route, onNavigate }: NavRailProps) {
  return (
    <nav className="nav-rail">
      <div className="nav-rail__brand">
        <span className="nav-rail__eyebrow">Windows Local Build</span>
        <strong>PersonaPilot</strong>
        <p>Local fingerprint browser operations desk</p>
      </div>
      <div className="nav-rail__items">
        {ITEMS.map((item) => (
          <button
            key={item.route}
            className={`nav-rail__item ${
              item.route === route ? "nav-rail__item--active" : ""
            }`}
            onClick={() => onNavigate(item.route)}
            type="button"
          >
            <span>{item.label}</span>
            <small>{item.detail}</small>
          </button>
        ))}
      </div>
    </nav>
  );
}
