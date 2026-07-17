# DoctorProfile transactional-outbox rollout

The PostgreSQL `doctor_profile_event_outbox` is the delivery authority. Each
approval, consultation-config change, deactivation, and reconciliation entry
commits a complete immutable V2 payload in the same transaction as its profile
mutation. `eventId` is stable across immediate delivery and relay retries.

The DoctorApp request path keeps `service.doctor_profile_immediate_delivery_enabled=true`
by default. After commit it leases the persisted event, publishes that exact
payload to `doctor-profile.approved` within a five-second timeout, and marks
the row published only on success. Failed or timed-out attempts clear their
lease and leave the row pending. This preserves origin/main delivery during
rollout without bypassing durability.

Deployment order:

1. Apply migration `20260713000000_expand_doctor_profile_approved_event_fields.sql`, then
   `20260713000001_doctor_profile_event_outbox.sql`.
2. Deploy DoctorApp with immediate delivery left enabled. Verify committed outbox
   rows and Pub/Sub `eventId`/`profileVersion` telemetry.
3. Deploy server-bg with PostgreSQL credentials and
   `DOCTOR_PROFILE_OUTBOX__ENABLED=true`. Verify relay retry and backlog drain.
4. Only after that verification, optionally set
   `SERVICE__DOCTOR_PROFILE_IMMEDIATE_DELIVERY_ENABLED=false` to use relay-only
   delivery. Never disable both paths.

Rollback: re-enable immediate delivery first. It leases only unpublished rows,
so it safely drains rows left by a stopped relay. Do not delete outbox rows or
reuse `eventId`s; the relay enforces ascending `profileVersion` per doctor.
