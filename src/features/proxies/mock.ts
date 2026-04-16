import type { ProxyDetailSnapshot, ProxyRowModel } from "./model";

function toEpochSeconds(value: string): string {
  return String(Math.floor(new Date(value).getTime() / 1000));
}

function fallbackRotation(
  sessionKey: string | null,
  requestedProvider: string | null,
  requestedRegion: string | null,
  expiresAt: string | null,
) {
  return {
    residencyStatus: sessionKey ? "sticky_active" : "stateless_rotation",
    rotationMode: sessionKey ? "sticky_refresh" : "provider_aware_rotate",
    sessionKey,
    requestedProvider,
    requestedRegion,
    stickyTtlSeconds: null,
    expiresAt,
    note: "Fallback snapshot does not execute provider-side rotation.",
    trackingTaskId: null,
  } as const;
}

const FALLBACK_PROXY_ROWS: ProxyRowModel[] = [
  {
    id: "proxy-us-resi-01",
    name: "US Residential Warm Pool",
    endpoint: "us-resi-warm.personapilot.local",
    port: 19001,
    protocol: "socks5",
    source: "provider",
    sourceLabel: "Provider sync",
    providerLabel: "Bright Data",
    authLabel: "Username + password",
    tags: ["residential", "us", "warm-pool"],
    note: "Shared by checkout and ad verification profiles.",
    exitIp: "104.28.45.12",
    regionLabel: "US / Ashburn",
    usageCount: 3,
    activeUsageCount: 2,
    usageLinks: [
      {
        id: "usage-1",
        profileId: "profile-west-01",
        profileName: "US Checkout Runner",
        groupName: "Checkout",
        profileStatus: "running",
        assignedAt: toEpochSeconds("2026-04-15T18:10:00+08:00"),
      },
      {
        id: "usage-2",
        profileId: "profile-west-02",
        profileName: "Ad Verify West",
        groupName: "QA Ads",
        profileStatus: "ready",
        assignedAt: toEpochSeconds("2026-04-15T17:45:00+08:00"),
      },
    ],
    rotation: fallbackRotation(
      "sess-us-resi-01",
      "Bright Data",
      "us-ashburn",
      toEpochSeconds("2026-04-16T03:10:00+08:00"),
    ),
    health: {
      state: "healthy",
      summary: "Healthy / 168ms",
      lastCheckAt: toEpochSeconds("2026-04-15T21:06:00+08:00"),
      latencyMs: 168,
      exitIp: "104.28.45.12",
      regionLabel: "US / Ashburn",
      failureReason: null,
      batchState: "completed",
    },
    lastUpdatedAt: toEpochSeconds("2026-04-15T21:06:00+08:00"),
  },
  {
    id: "proxy-de-dc-02",
    name: "DE Datacenter Burst",
    endpoint: "de-dc-burst.personapilot.local",
    port: 19012,
    protocol: "http",
    source: "imported",
    sourceLabel: "CSV import",
    providerLabel: "Netnut",
    authLabel: "IP whitelist",
    tags: ["datacenter", "de", "burst"],
    note: "Import batch pending fresh exit-IP verification.",
    exitIp: "91.239.18.76",
    regionLabel: "DE / Frankfurt",
    usageCount: 1,
    activeUsageCount: 0,
    usageLinks: [
      {
        id: "usage-4",
        profileId: "profile-de-01",
        profileName: "Marketplace Probe DE",
        groupName: "Market QA",
        profileStatus: "paused",
        assignedAt: toEpochSeconds("2026-04-15T12:35:00+08:00"),
      },
    ],
    rotation: fallbackRotation(
      "sess-de-dc-02",
      "Netnut",
      "de-frankfurt",
      toEpochSeconds("2026-04-16T01:30:00+08:00"),
    ),
    health: {
      state: "warning",
      summary: "High latency / 842ms",
      lastCheckAt: toEpochSeconds("2026-04-15T20:52:00+08:00"),
      latencyMs: 842,
      exitIp: "91.239.18.76",
      regionLabel: "DE / Frankfurt",
      failureReason: null,
      batchState: "completed",
    },
    lastUpdatedAt: toEpochSeconds("2026-04-15T20:52:00+08:00"),
  },
  {
    id: "proxy-jp-shared-02",
    name: "JP Shared Rotation",
    endpoint: "jp-shared-rotation.personapilot.local",
    port: 19144,
    protocol: "socks5",
    source: "shared",
    sourceLabel: "Shared pool",
    providerLabel: "Oxylabs",
    authLabel: "Username + password",
    tags: ["shared", "jp", "rotation"],
    note: "Assigned to seasonal browsing profiles but currently unstable.",
    exitIp: null,
    regionLabel: null,
    usageCount: 2,
    activeUsageCount: 0,
    usageLinks: [
      {
        id: "usage-7",
        profileId: "profile-jp-01",
        profileName: "Seasonal Crawl JP",
        groupName: "Research",
        profileStatus: "paused",
        assignedAt: toEpochSeconds("2026-04-13T11:40:00+08:00"),
      },
    ],
    rotation: fallbackRotation(null, "Oxylabs", "jp-tokyo", null),
    health: {
      state: "failed",
      summary: "Handshake timeout",
      lastCheckAt: toEpochSeconds("2026-04-15T19:38:00+08:00"),
      latencyMs: null,
      exitIp: null,
      regionLabel: null,
      failureReason: "Proxy handshake timed out during prior smoke check.",
      batchState: "completed",
    },
    lastUpdatedAt: toEpochSeconds("2026-04-15T19:38:00+08:00"),
  },
];

export function createFallbackProxyRows(): ProxyRowModel[] {
  return FALLBACK_PROXY_ROWS.map((row) => ({
    ...row,
    tags: [...row.tags],
    usageLinks: row.usageLinks.map((link) => ({
      ...link,
    })),
    health: {
      ...row.health,
    },
  }));
}

export function buildFallbackProxyDetail(proxyId: string): ProxyDetailSnapshot | null {
  const row = FALLBACK_PROXY_ROWS.find((item) => item.id === proxyId);
  if (!row) {
    return null;
  }

  return {
    proxyId,
    health: {
      ...row.health,
    },
    usageLinks: row.usageLinks.map((link) => ({
      ...link,
    })),
    rotation: {
      ...row.rotation,
    },
    source: "fallback",
  };
}
