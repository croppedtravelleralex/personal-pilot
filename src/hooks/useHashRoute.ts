import { useCallback, useEffect, useState } from "react";

export type AppRoute =
  | "dashboard"
  | "profiles"
  | "proxies"
  | "automation"
  | "synchronizer"
  | "logs"
  | "settings";

const VALID_ROUTES: AppRoute[] = [
  "dashboard",
  "profiles",
  "proxies",
  "automation",
  "synchronizer",
  "logs",
  "settings",
];

const LEGACY_ROUTE_MAP: Record<string, AppRoute> = {
  overview: "dashboard",
  tasks: "automation",
  task: "automation",
};

function resolveRoute(hash: string): AppRoute {
  const route = hash.replace(/^#/, "").trim();
  const normalizedRoute = LEGACY_ROUTE_MAP[route] ?? route;
  return (VALID_ROUTES.find((item) => item === normalizedRoute) ??
    "dashboard") as AppRoute;
}

export function useHashRoute() {
  const [route, setRoute] = useState<AppRoute>(() => resolveRoute(window.location.hash));

  useEffect(() => {
    const handleHashChange = () => {
      setRoute(resolveRoute(window.location.hash));
    };

    window.addEventListener("hashchange", handleHashChange);
    return () => {
      window.removeEventListener("hashchange", handleHashChange);
    };
  }, []);

  const navigate = useCallback((nextRoute: AppRoute) => {
    const nextHash = nextRoute === "dashboard" ? "" : `#${nextRoute}`;
    if (window.location.hash !== nextHash) {
      window.location.hash = nextHash;
    } else {
      setRoute(nextRoute);
    }
  }, []);

  return {
    route,
    navigate,
  };
}
