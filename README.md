# PersonaPilot

This root `README.md` is now a compatibility entrypoint.
Canonical maintenance docs live under `/docs`.

## Current Reporting Rule

Use the `2026-04-15` reporting split everywhere:

- `runtime alive`
- `build status`
- `verification / acceptance status`

Do not treat legacy `93%` stage-closeout wording or historical `real-upstream` wording as the current truth source.

## Read Order

1. `/docs/README.md`
2. `/docs/02-current-state.md`
3. `/docs/03-roadmap.md`
4. `/docs/04-improvement-backlog.md`
5. `/docs/05-ai-maintenance-playbook.md`
6. `/docs/root-entrypoint-map.md`

## Current Runtime Summary

- control plane health can be online while build/test closure is still incomplete
- gateway health can be online while `upstream_configured=false`
- heavy `verify_proxy` traffic is not accepted as proof of browser mainline closure

