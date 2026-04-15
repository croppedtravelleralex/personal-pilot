# Proxy Real-Live Driver

## Goal

Provide one repo-owned entry that can do all of the following in one run:

- force a real-source harvest into the shared SQLite proxy registry
- keep generating browser traffic so hot regions, site history, and continuity counters can move
- keep sampling `/status`
- write a final long-run report with pool, continuity, and site deltas

This is meant to move the project from:

- "we can sample `/status`"

to:

- "we can actively drive a 30min+ real-source validation window"

## Entry points

Preferred entry:

```bash
bash scripts/proxy_mainline_verify.sh real-live
```

Direct entry:

```bash
python3 scripts/proxy_real_longrun_driver.py \
  --mode prod_live \
  --config /absolute/path/to/proxy_sources.real.json \
  --fingerprint-profile-id fp-global-desktop-chrome \
  --warm-url https://example.com \
  --warm-url https://httpbin.org/ip \
  --auto-browser-regions-from-db \
  --max-browser-regions 3
```

## Continuity requirement

If you want this driver to exercise identity/session continuity instead of only proxy demand, you must pass a valid fingerprint profile id.

Without `--fingerprint-profile-id`:

- browser tasks still generate proxy demand
- site metrics may still move
- `identity_session_status` remains `not_applicable`
- auto session reuse will not be exercised

With a valid fingerprint profile id and repeated visits to the same host:

- `identity_session_status` can move through `auto_created` and `auto_reused`
- `/status` continuity counters can grow
- the final report can confirm real session reuse instead of only task success

## Mode rule

脚本现在支持 `--mode`，推荐和运行中的控制面 mode 保持一致：

- `demo_public`
- `prod_live`

推荐：

```bash
PERSONA_PILOT_PROXY_MODE=prod_live \
bash scripts/proxy_mainline_verify.sh real-live
```

或直接：

```bash
python3 scripts/proxy_real_longrun_driver.py --mode prod_live ...
```

## Required config rule

`real-live` should use a real external config file, not the repo seed/demo files.

Recommended:

- keep the real config outside the repo working tree
- set `PERSONA_PILOT_PROXY_HARVEST_CONFIG=/absolute/path/to/proxy_sources.real.json`

The driver rejects:

- `data/proxy_sources.json`
- `data/proxy_sources.demo.json`

unless `--allow-demo-config` or `PROXY_VERIFY_REAL_ALLOW_DEMO=1` is explicitly set.

## Real config path behavior

`scripts/proxy_harvest_mvp.py` now supports:

- config files stored outside the repo
- relative `config_json.path` values resolved against the config file directory
- full-line `#` comments in the config file

That means a real config can live beside its own private source files, for example:

```json
[
  {
    "source_label": "real_primary_text",
    "source_kind": "text_file",
    "enabled": true,
    "interval_seconds": 300,
    "base_proxy_score": 1.2,
    "config_json": {
      "path": "./real_proxy_candidates_primary.txt"
    }
  }
]
```

If the config file is `/srv/lightpanda/proxy_sources.real.json`, the relative path above resolves to:

- `/srv/lightpanda/real_proxy_candidates_primary.txt`

## Useful environment variables

- `PROXY_VERIFY_REAL_CONFIG`
- `PROXY_VERIFY_REAL_DURATION_SECONDS`
- `PROXY_VERIFY_REAL_HARVEST_INTERVAL_SECONDS`
- `PROXY_VERIFY_REAL_GEO_ENRICH_INTERVAL_SECONDS`
- `PROXY_VERIFY_REAL_GEO_ENRICH_LIMIT`
- `PROXY_VERIFY_REAL_DISABLE_GEO_ENRICH`
- `PROXY_VERIFY_REAL_STATUS_INTERVAL_SECONDS`
- `PROXY_VERIFY_REAL_BROWSER_INTERVAL_SECONDS`
- `PROXY_VERIFY_REAL_BROWSER_TIMEOUT_SECONDS`
- `PROXY_VERIFY_REAL_BROWSER_ENDPOINT`
- `PROXY_VERIFY_REAL_WARM_URLS`
- `PROXY_VERIFY_REAL_BROWSER_REGION`
- `PROXY_VERIFY_REAL_AUTO_BROWSER_REGIONS_FROM_DB`
- `PROXY_VERIFY_REAL_MAX_BROWSER_REGIONS`
- `PROXY_VERIFY_REAL_FINGERPRINT_PROFILE_ID`
- `PERSONA_PILOT_API_KEY`

## Outputs

`real-live` writes:

- `reports/proxy_real_longrun_driver_latest.json`
- `reports/proxy_real_longrun_latest.txt`
- `reports/proxy_real_longrun_latest.json`

The final report now includes:

- active/promotion/reject/fallback aggregates
- effective vs reported active ratio percent
- continuity deltas
- site deltas
- recent driver event summaries
- browser identity-session status counts
- browser requested regions
- browser hot regions observed during in-flight tasks
- geo enrich region coverage after the last enrichment window

## Geo enrichment and region demand

`prod_live` 现在支持两条额外提分链路：

1. **geo enrichment**
   - 周期性调用 `scripts/prod_proxy_geo_enrich.py`
   - 用 host-IP geolocation 补 `country / region / last_exit_country / last_exit_region`
   - 不需要重编控制面 binary

2. **auto browser regions from db**
   - 从 SQLite 当前 active pool 里读取 top-N region
   - 把这些 region 轮转注入 `network_policy_json.region`
   - 用于把真实热区 demand 打进 browser tasks

注意：

- latest `/status.proxy_pool_status.hot_regions` 只看 queued/running 任务，任务完成后可能回到空
- 因此 longrun report 现在额外记录：
  - `browser_requested_regions`
  - `browser_hot_regions_observed`

这样即使 latest snapshot 归零，也能证明 in-flight region demand 已真实出现

## Current boundary

This driver does not replace the in-process replenish loop.

Instead it helps the existing mainline by:

- forcing fresh real-source harvest runs
- repeatedly creating browser demand
- capturing whether pool, site, and continuity signals are actually moving
