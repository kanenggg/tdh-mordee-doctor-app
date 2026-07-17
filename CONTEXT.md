# DoctorApp Profile Context

This context defines DoctorApp-owned doctor-profile and onboarding language. It is intentionally implementation-free.

## Language

**Doctor Consultation Configuration**:
The Doctor Profile-owned service configuration that determines the consultation services a doctor is eligible to provide: supported channels, supported languages, duration, fee, currency, and related profile eligibility.
_Avoid_: Treating APMv2 or Walrus projections as editable sources for this configuration.

**Doctor Profile Approved Snapshot**:
The complete, versioned `DoctorProfileApproved` fact emitted after DoctorApp approves and persists a Doctor Profile and its Doctor Consultation Configuration. It includes the top-level fee, currency, languages, duration, and channels needed by downstream projections.
_Avoid_: Requiring a consumer to reconstruct this snapshot from APMv2 or from a nested extension field.

**Doctor Review Queue**:
The backoffice work queue of Doctor Profile Drafts in `PendingApproval` awaiting a reviewer decision. A queued draft is not an Approved Doctor and is never included in the Doctor Directory.
_Avoid_: Calling this collection a doctor list without its review state, or treating it as a projection of active profiles.

**Approved Doctor Directory**:
The read model of active, approved Doctor Profiles. It supports privileged list and detail lookup, but does not expose review-only identity documents or act as an editable source for Doctor Consultation Configuration.
_Avoid_: Reusing the legacy Firestore onboarding collection as the directory's authority.

**Doctor Profile Approval**:
The reviewer-authorized lifecycle transition that moves a Doctor Profile Draft from `PendingApproval` to an active Approved Doctor Profile, atomically persisting its Doctor Consultation Configuration and Approved Snapshot outbox record.
_Avoid_: Treating approval as a generic profile update or a client-side status change.

**Doctor Operational Availability**:
The Scheduling-owned state that determines when a doctor can be held or booked, including schedule configuration, availability modes, Holds, and Doctor Occupancy. APMv2 owns this state after consuming the approved-profile snapshot.
_Avoid_: Editing operational availability as part of Doctor Profile configuration.

## Relationships

- DoctorApp is the sole authority for Doctor Consultation Configuration.
- DoctorApp emits the complete **Doctor Profile Approved Snapshot** directly to APMv2 and Walrus through independent projections.
- APMv2 uses the projected duration and channels to generate and validate Timeslots, but does not republish Doctor Consultation Configuration as a source fact.
- Walrus displays the DoctorApp-projected duration and combines it with APMv2-owned availability snapshots for discovery; it is never booking authority.
