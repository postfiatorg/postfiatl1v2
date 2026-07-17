# Incident Response

**Scope:** controlled pre-testnet operations. This policy does not claim a
production pager, independent control plane, or real-value launch readiness.

## Machine contract

`scripts/testnet-monitor-snapshot --alert-spool-dir PATH` emits private,
idempotent `postfiat.monitor-alert.v1` envelopes. Every envelope carries the
ordered monitor signals and one of these response policies:

| Monitor state | Incident class | Acknowledge | Incident commander | First public update |
| --- | --- | ---: | ---: | ---: |
| critical | SEV-1 | 5 min | 10 min | 30 min |
| warning | SEV-2 | 15 min | 30 min | 120 min |

## Controlled pre-testnet SLOs

Evaluate these over a rolling 30-day window and retain the underlying signed or
content-addressed snapshots. They are objectives, not evidence that the current
deployment has achieved them.

| Objective | Target | Immediate breach behavior |
| --- | ---: | --- |
| Monitor collection freshness | 99.9% of scheduled 60-second samples complete within 120 seconds | missing sample is SEV-1 |
| Height/root and registry agreement | 100% of complete fleet samples | any disagreement is SEV-1; stop writes |
| Receipt semantic classification | 100%; unknown count is zero | unknown is SEV-1 |
| Read RPC latency | p95 at or below 1,000 ms per collection | warning above 1,000 ms; SEV-1 above 5,000 ms |
| RPC connection headroom | below 750,000 ppm active | warning at 750,000 ppm; SEV-1 at 950,000 ppm |
| Proof verification | latest supported proof at or below 5,000 ms and fresher than 5 minutes | warning above 5,000 ms; SEV-1 above 15,000 ms or stale |
| Disk headroom | above 150,000 ppm available | warning at/below 150,000 ppm; SEV-1 at/below 50,000 ppm or unavailable |
| Critical alert delivery | 99.9% delivered and acknowledged within 5 minutes | delivery or acknowledgement miss is a separate SEV-1 |
| Public incident update | 100% of user/safety-impacting SEV-1 incidents within 30 minutes | missed update is included in the incident review |

An external delivery agent must atomically claim events from the private spool,
deliver them to the configured primary on-call, record provider acknowledgement
and delivery latency, and escalate to the backup within the envelope deadline.
The repository does not embed pager credentials or execute arbitrary shell
hooks. A spooled event is not a delivered page.

## SEV-1

Treat any critical monitor condition as safety or availability unknown until
disproved. The primary on-call acknowledges within five minutes, pages the
backup immediately if acknowledgement is absent, and assigns an incident
commander within ten minutes. Stop write admission for root divergence,
unknown receipt semantics, below-quorum reachability, missing disk/saturation
telemetry, critical disk/proof/RPC/clock thresholds, or doctor failure. Preserve
logs and snapshot evidence before any restart. Never reset or rewrite shared
state to clear an alert.

Publish a plain-language status update within 30 minutes when any user-facing
service, funds path, finality claim, or safety assumption may be affected. State
what is known, what is unknown, affected surfaces, containment, and the next
update time. Do not include validator addresses, keys, tokens, raw notes, or
private topology.

## SEV-2

Warnings require acknowledgement within 15 minutes and backup escalation if
unacknowledged. Assign an incident commander within 30 minutes when the warning
persists for two collection intervals, affects multiple validators, or has user
impact. Investigate without weakening thresholds. Publish within 120 minutes
when users are affected or the warning persists; otherwise retain a redacted
internal incident record.

## Closure evidence

Close an incident only after the triggering signal is normal for two collection
intervals, fleet height/root and receipt semantics agree, the corrective action
has a recorded owner, and a redacted timeline is preserved. SEV-1 requires a
post-incident review with reproduction, root cause, corrective tests, and a
public follow-up when a public status update was issued.

Before any production claim, run delivery-failure, delayed-acknowledgement,
multi-region partition, disk-loss, restore, replacement, rollback, and
credential-loss drills under operators who do not share a founder account,
host, provider, or control plane.
