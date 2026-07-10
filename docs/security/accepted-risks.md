# Accepted Risks

> Risks that have been reviewed and deliberately accepted rather than mitigated.
> Each entry is a traceable decision — not a forgotten issue.
> Risks that meet the ADR criteria (hard to reverse, surprising, real trade-off) go in docs/adr/ instead.

| ID | Risk | Rationale for acceptance | Review date | Owner |
|-----|------|--------------------------|-------------|-------|
| AR-001 | **No authentication or authorization.** Every route is fully open to any client that can reach the bound socket. | Single-operator, self-hosted tool on a trusted network (localhost or a private LAN). No multi-tenancy, no PII, no public sign-up. Adding auth is out of scope for v0–v1. **Compensating control:** deploy bound to `127.0.0.1` or a trusted interface, behind a reverse proxy that enforces TLS + auth before any exposure beyond localhost. Revisit before any internet exposure. | 2026-07-10 | ziggornif |
| AR-002 | **Internal error detail leaks to the client.** 500-path handlers return `e.to_string()` (incl. raw sqlx text) in the response body. | Low impact under AR-001 (the only client is the operator). Accepted as an interim state; the recommended fix (generic client message + server-side `tracing::error!`) is a small follow-up, not a blocker for current use. | 2026-07-10 | ziggornif |
| AR-003 | **Unbounded list queries** (spools/materials have no pagination). | Bounded in practice — a personal/micro-business filament inventory is hundreds of rows, not millions. Latent scaling concern, not a live DoS. Add pagination if inventory size ever warrants it. | 2026-07-10 | ziggornif |
| AR-004 | **No audit log** of stock mutations. | Single operator; no second party to dispute an action, so repudiation has no meaning here. Reconsider if the tool ever gains multiple users. | 2026-07-10 | ziggornif |

> AR-001 is the load-bearing decision. It is recorded here rather than as an ADR
> because it is cheap to reverse (put a proxy in front, or add a middleware layer)
> and follows directly from the single-user deployment model in the product brief —
> it's a deployment constraint, not an architectural trade-off. Promote it to an ADR
> if a decision is ever made to build auth *into* the app.
