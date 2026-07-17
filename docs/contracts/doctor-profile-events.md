# Doctor Profile Pub/Sub Event Specification

**Owner:** TDH Doctor App

**Transport:** Google Cloud Pub/Sub

**Payload encoding:** UTF-8 JSON in `PubsubMessage.data`

## Scope

This document defines the current downstream contracts for two doctor-profile
events:

| Topic | Event type | Schema version | Meaning |
|---|---|---:|---|
| `doctor-profile.approved` | `DoctorProfileApproved` | 2 | Complete approved/active doctor projection and consultation-configuration snapshot |
| `doctor-profile.status-updated` | `DoctorProfileStatusUpdated` | 3 | Resulting active state after an actual activation or deactivation transition |

The examples use synthetic data and do not represent a real doctor.

## Common Pub/Sub message contract

Both topics use the same Pub/Sub metadata convention:

| Location | Field | Contract |
|---|---|---|
| Attribute | `eventType` | Same string as payload `__type` |
| Attribute | `eventId` | Same UUID as payload `eventId` |
| Attribute | `schemaVersion` | Decimal string matching the integer payload `schemaVersion` |
| Message | `orderingKey` | Decimal string representation of payload `doctorAccountId` |
| Message | `data` | UTF-8 JSON bytes; represented as Base64 text by the Pub/Sub REST API and `gcloud --format=json` |
| Message | `messageId` | Assigned by Google Cloud Pub/Sub; not part of domain idempotency |
| Message | `publishTime` | Assigned by Google Cloud Pub/Sub; not the domain occurrence time |

For example, a pulled REST message has this outer shape:

```json
{
  "message": {
    "data": "<Base64-encoded JSON payload>",
    "attributes": {
      "eventType": "<payload __type>",
      "eventId": "<payload eventId>",
      "schemaVersion": "<payload schemaVersion as a string>"
    },
    "messageId": "12345678901234567",
    "orderingKey": "2443",
    "publishTime": "2026-07-16T03:47:50Z"
  }
}
```

The angle-bracket values above describe the envelope shape; complete concrete
messages are provided under each event.

## Pub/Sub request and response schemas

The schemas in this section describe the Pub/Sub transport envelope shared by
both topics. The Base64-decoded `data` object must also satisfy the
topic-specific event schema later in this document.

### Topic values

| Topic | `eventType` / payload `__type` | `schemaVersion` | `orderingKey` |
|---|---|---:|---|
| `doctor-profile.approved` | `DoctorProfileApproved` | `2` | Decimal payload `doctorAccountId` |
| `doctor-profile.status-updated` | `DoctorProfileStatusUpdated` | `3` | Decimal payload `doctorAccountId` |

### Publish request schema

This Draft 2020-12 schema models the request body sent to Pub/Sub. It validates
the Base64 transport data and topic metadata; it does not validate the decoded
event payload.

[Open the standalone JSON Schema](schemas/doctor-profile-publish-request.schema.json)

### Pull response schema

This schema models a pull response containing delivered messages. `ackId` is
transport state used to acknowledge a delivery; it is not a domain identifier
or an event-specific business response. `deliveryAttempt` is optional and is
normally populated only when dead-letter delivery tracking applies.

[Open the standalone JSON Schema](schemas/doctor-profile-pull-response.schema.json)

## Delivery, ordering, idempotency, and versioning

- Treat delivery as **at least once**. Pub/Sub delivery and outbox recovery can
  produce duplicates.
- Use payload `eventId`, not Pub/Sub `messageId`, as the durable idempotency key.
  The producer preserves `eventId` across retries.
- `profileVersion` is monotonically increasing for one `doctorId`. Store the
  greatest applied version and ignore an event whose version is equal to or
  lower than that value.
- The producer sets `orderingKey` to `doctorAccountId`, but consumers must not
  use delivery order as a substitute for `profileVersion` checks.
- Domain timestamps are integer Unix epoch seconds in UTC. Use `occurredAt` for
  the domain event time, not Pub/Sub `publishTime`.
- Check that attribute `eventType`, payload `__type`, and the supported
  `schemaVersion` agree before processing.
- Tolerate unknown additive JSON fields so compatible producer evolution does
  not break consumers.
- Acknowledge a message only after the consumer's processing result and
  idempotency record are durable.

## Topic: `doctor-profile.approved`

### Event description

`DoctorProfileApproved` is a complete projection of an approved, active doctor
and the doctor's current consultation configuration. It is not merely a
one-time notification that approval occurred. A downstream projection consumer
can replace its stored doctor view with the newer event when `profileVersion`
is greater than the version it has already applied.

The event can be produced by:

- initial doctor-profile approval;
- consultation-configuration changes;
- doctor profile-configuration changes; and
- reconciliation of an active profile whose current projection event is
  missing.

`approvedAt` represents the original approval/profile creation time.
`occurredAt` represents when this particular projection event was created.

### Payload field definitions

| Field | JSON type | Required | Definition |
|---|---|---:|---|
| `__type` | string | Yes | Constant `DoctorProfileApproved` |
| `eventId` | UUID string | Yes | Stable idempotency identifier for this event |
| `doctorId` | UUID string | Yes | Canonical doctor aggregate identifier; recommended projection key |
| `doctorAccountId` | integer | Yes | Doctor identity/account identifier; also serialized as the ordering key |
| `doctorProfileId` | integer | Yes | Doctor profile identifier from the identity/profile domain |
| `departmentId` | integer | Yes | Department identifier |
| `department` | `Localized` | Yes | Localized department name |
| `counselingAreas` | array of `Localized` | Yes | Localized counseling areas; may be empty |
| `isActive` | boolean | Yes | Active state in this projection; current approved-event triggers emit active profiles |
| `profession` | `Profession` | Yes | Professional title and abbreviations |
| `specialty` | `Specialty` | Yes | Primary specialty snapshot |
| `workPlace` | array of `WorkPlace` | Yes | Workplace snapshots; may be empty |
| `academicPosition` | `AcademicPosition` | Yes | Academic title and abbreviations |
| `firstName` | `Localized` | Yes | Localized first name |
| `lastName` | `Localized` | Yes | Localized last name |
| `profileImageUrl` | string | Yes | Doctor profile-image location; consumers must apply their normal data-access policy |
| `approvedAt` | integer | Yes | Original approval/profile creation time as Unix epoch seconds |
| `occurredAt` | integer | Yes | Event creation time as Unix epoch seconds |
| `schemaVersion` | integer | Yes | Constant `2` for this payload contract |
| `profileVersion` | integer | Yes | Monotonic version for this doctor; minimum `1` |
| `consultationConfig` | `ConsultationConfig` | Yes | Canonical, complete consultation-configuration snapshot |

`consultationConfig` is the event's only consultation-configuration
projection. Its nested fields contain the fee, currency, languages, duration,
and channels.

### Nested object definitions

| Object | Field | JSON type | Required | Definition |
|---|---|---|---:|---|
| `Localized` | `th` | string | Yes | Thai value; may be an empty string when source reference data is unavailable |
| `Localized` | `en` | string | Yes | English value; may be an empty string when source reference data is unavailable |
| `Profession` | `id` | integer | Yes | Profession identifier |
| `Profession` | `name` | `Localized` | Yes | Localized profession name |
| `Profession` | `abbr` | `Localized` | Yes | Localized profession abbreviation |
| `AcademicPosition` | `id` | integer | Yes | Academic-position identifier |
| `AcademicPosition` | `name` | `Localized` | Yes | Localized academic-position name |
| `AcademicPosition` | `abbr` | `Localized` | Yes | Localized academic-position abbreviation |
| `WorkPlace` | `id` | integer | Yes | Workplace identifier |
| `WorkPlace` | `name` | `Localized` | Yes | Localized workplace name |
| `MedicalSchool` | `id` | integer | Yes | Medical-school identifier |
| `MedicalSchool` | `name` | `Localized` | Yes | Localized medical-school name |
| `Specialty` | `id` | integer | Yes | Specialty identifier |
| `Specialty` | `name` | `Localized` | Yes | Localized specialty name |
| `Specialty` | `subspecialty` | `Specialty` | No | Nested subspecialty; omitted when absent |
| `Specialty` | `medicalSchool` | `MedicalSchool` | Yes | Associated medical-school snapshot |
| `ConsultationConfig` | `channels` | array | Yes | Non-empty unique set containing `voice`, `chat`, and/or `video` |
| `ConsultationConfig` | `languages` | array | Yes | Non-empty unique set containing `th` and/or `en` |
| `ConsultationConfig` | `durationMinutes` | integer | Yes | One of `15`, `25`, or `50` |
| `ConsultationConfig` | `feeAmount` | string | Yes | Non-negative fixed-scale amount with exactly two decimal places |
| `ConsultationConfig` | `currency` | string | Yes | Non-empty currency code; currently `THB` |

### JSON payload schema

[Open the standalone JSON Schema](schemas/doctor-profile-approved.v2.schema.json)

### Decoded example payload

```json
{
  "__type": "DoctorProfileApproved",
  "eventId": "11111111-1111-4111-8111-111111111111",
  "doctorId": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "doctorAccountId": 2443,
  "doctorProfileId": 8891,
  "departmentId": 17,
  "department": {
    "th": "อายุรกรรม",
    "en": "Internal Medicine"
  },
  "counselingAreas": [
    {
      "th": "หัวใจ",
      "en": "Heart"
    }
  ],
  "isActive": true,
  "profession": {
    "id": 1,
    "name": {
      "th": "แพทย์",
      "en": "Doctor"
    },
    "abbr": {
      "th": "พญ.",
      "en": "Dr."
    }
  },
  "specialty": {
    "id": 10,
    "name": {
      "th": "โรคหัวใจ",
      "en": "Cardiology"
    },
    "medicalSchool": {
      "id": 12,
      "name": {
        "th": "มหาวิทยาลัยแพทย์",
        "en": "Medical University"
      }
    }
  },
  "workPlace": [
    {
      "id": 20,
      "name": {
        "th": "โรงพยาบาลตัวอย่าง",
        "en": "Example Hospital"
      }
    }
  ],
  "academicPosition": {
    "id": 2,
    "name": {
      "th": "อาจารย์",
      "en": "Lecturer"
    },
    "abbr": {
      "th": "อ.",
      "en": "Lect."
    }
  },
  "firstName": {
    "th": "ทดสอบ",
    "en": "Test"
  },
  "lastName": {
    "th": "แพทย์",
    "en": "Doctor"
  },
  "profileImageUrl": "https://example.invalid/doctors/f47ac10b/profile.jpg",
  "approvedAt": 1784106000,
  "occurredAt": 1784106470,
  "schemaVersion": 2,
  "profileVersion": 1,
  "consultationConfig": {
    "channels": ["voice", "chat"],
    "languages": ["th", "en"],
    "durationMinutes": 15,
    "feeAmount": "650.00",
    "currency": "THB"
  }
}
```

### Pub/Sub message example

`data` below is the Base64 encoding of the compact JSON form of the decoded
example above.

```json
{
  "message": {
    "data": "eyJfX3R5cGUiOiJEb2N0b3JQcm9maWxlQXBwcm92ZWQiLCJldmVudElkIjoiMTExMTExMTEtMTExMS00MTExLTgxMTEtMTExMTExMTExMTExIiwiZG9jdG9ySWQiOiJmNDdhYzEwYi01OGNjLTQzNzItYTU2Ny0wZTAyYjJjM2Q0NzkiLCJkb2N0b3JBY2NvdW50SWQiOjI0NDMsImRvY3RvclByb2ZpbGVJZCI6ODg5MSwiZGVwYXJ0bWVudElkIjoxNywiZGVwYXJ0bWVudCI6eyJ0aCI6IuC4reC4suC4ouC4uOC4o+C4geC4o+C4o+C4oSIsImVuIjoiSW50ZXJuYWwgTWVkaWNpbmUifSwiY291bnNlbGluZ0FyZWFzIjpbeyJ0aCI6IuC4q+C4seC4p+C5g+C4iCIsImVuIjoiSGVhcnQifV0sImlzQWN0aXZlIjp0cnVlLCJwcm9mZXNzaW9uIjp7ImlkIjoxLCJuYW1lIjp7InRoIjoi4LmB4Lie4LiX4Lii4LmMIiwiZW4iOiJEb2N0b3IifSwiYWJiciI6eyJ0aCI6IuC4nuC4jS4iLCJlbiI6IkRyLiJ9fSwic3BlY2lhbHR5Ijp7ImlkIjoxMCwibmFtZSI6eyJ0aCI6IuC5guC4o+C4hOC4q+C4seC4p+C5g+C4iCIsImVuIjoiQ2FyZGlvbG9neSJ9LCJtZWRpY2FsU2Nob29sIjp7ImlkIjoxMiwibmFtZSI6eyJ0aCI6IuC4oeC4q+C4suC4p+C4tOC4l+C4ouC4suC4peC4seC4ouC5geC4nuC4l+C4ouC5jCIsImVuIjoiTWVkaWNhbCBVbml2ZXJzaXR5In19fSwid29ya1BsYWNlIjpbeyJpZCI6MjAsIm5hbWUiOnsidGgiOiLguYLguKPguIfguJ7guKLguLLguJrguLLguKXguJXguLHguKfguK3guKLguYjguLLguIciLCJlbiI6IkV4YW1wbGUgSG9zcGl0YWwifX1dLCJhY2FkZW1pY1Bvc2l0aW9uIjp7ImlkIjoyLCJuYW1lIjp7InRoIjoi4Lit4Liy4LiI4Liy4Lij4Lii4LmMIiwiZW4iOiJMZWN0dXJlciJ9LCJhYmJyIjp7InRoIjoi4LitLiIsImVuIjoiTGVjdC4ifX0sImZpcnN0TmFtZSI6eyJ0aCI6IuC4l+C4lOC4quC4reC4miIsImVuIjoiVGVzdCJ9LCJsYXN0TmFtZSI6eyJ0aCI6IuC5geC4nuC4l+C4ouC5jCIsImVuIjoiRG9jdG9yIn0sInByb2ZpbGVJbWFnZVVybCI6Imh0dHBzOi8vZXhhbXBsZS5pbnZhbGlkL2RvY3RvcnMvZjQ3YWMxMGIvcHJvZmlsZS5qcGciLCJhcHByb3ZlZEF0IjoxNzg0MTA2MDAwLCJvY2N1cnJlZEF0IjoxNzg0MTA2NDcwLCJzY2hlbWFWZXJzaW9uIjoyLCJwcm9maWxlVmVyc2lvbiI6MSwiY29uc3VsdGF0aW9uQ29uZmlnIjp7ImNoYW5uZWxzIjpbInZvaWNlIiwiY2hhdCJdLCJsYW5ndWFnZXMiOlsidGgiLCJlbiJdLCJkdXJhdGlvbk1pbnV0ZXMiOjE1LCJmZWVBbW91bnQiOiI2NTAuMDAiLCJjdXJyZW5jeSI6IlRIQiJ9fQ==",
    "attributes": {
      "eventType": "DoctorProfileApproved",
      "eventId": "11111111-1111-4111-8111-111111111111",
      "schemaVersion": "2"
    },
    "messageId": "12345678901234567",
    "orderingKey": "2443",
    "publishTime": "2026-07-16T03:47:50Z"
  }
}
```

## Topic: `doctor-profile.status-updated`

### Event description

`DoctorProfileStatusUpdated` represents an actual transition of a doctor's
active state in either direction. Activation and deactivation use the same
payload shape. `isActive` is the resulting state after the committed update:

- `true`: the doctor became active;
- `false`: the doctor became inactive.

The event is produced only when the stored state changes. An idempotent request
for the already-current state produces no event and does not increment
`profileVersion`. The current producer assigns the same epoch-second value to
`statusUpdatedAt` and `occurredAt`.

### Payload field definitions

| Field | JSON type | Required | Definition |
|---|---|---:|---|
| `__type` | string | Yes | Constant `DoctorProfileStatusUpdated` |
| `eventId` | UUID string | Yes | Stable idempotency identifier for this event |
| `doctorId` | UUID string | Yes | Canonical doctor aggregate identifier |
| `doctorAccountId` | integer | Yes | Doctor identity/account identifier; also serialized as the ordering key |
| `doctorProfileId` | integer | Yes | Doctor profile identifier from the identity/profile domain |
| `isActive` | boolean | Yes | Resulting active state after the transition |
| `statusUpdatedAt` | integer | Yes | Status-transition time as Unix epoch seconds |
| `occurredAt` | integer | Yes | Domain event time as Unix epoch seconds; currently equal to `statusUpdatedAt` |
| `schemaVersion` | integer | Yes | Constant `3` for this payload contract |
| `profileVersion` | integer | Yes | Monotonic version for this doctor; minimum `1` |

### JSON payload schema

[Open the standalone JSON Schema](schemas/doctor-profile-status-updated.v3.schema.json)

### Decoded deactivation example

```json
{
  "__type": "DoctorProfileStatusUpdated",
  "eventId": "22222222-2222-4222-8222-222222222222",
  "doctorId": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "doctorAccountId": 2443,
  "doctorProfileId": 8891,
  "isActive": false,
  "statusUpdatedAt": 1784107000,
  "occurredAt": 1784107000,
  "schemaVersion": 3,
  "profileVersion": 2
}
```

### Decoded activation example

```json
{
  "__type": "DoctorProfileStatusUpdated",
  "eventId": "33333333-3333-4333-8333-333333333333",
  "doctorId": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "doctorAccountId": 2443,
  "doctorProfileId": 8891,
  "isActive": true,
  "statusUpdatedAt": 1784107600,
  "occurredAt": 1784107600,
  "schemaVersion": 3,
  "profileVersion": 3
}
```

### Pub/Sub message example

`data` below is the Base64 encoding of the compact JSON form of the decoded
deactivation example.

```json
{
  "message": {
    "data": "eyJfX3R5cGUiOiJEb2N0b3JQcm9maWxlU3RhdHVzVXBkYXRlZCIsImV2ZW50SWQiOiIyMjIyMjIyMi0yMjIyLTQyMjItODIyMi0yMjIyMjIyMjIyMjIiLCJkb2N0b3JJZCI6ImY0N2FjMTBiLTU4Y2MtNDM3Mi1hNTY3LTBlMDJiMmMzZDQ3OSIsImRvY3RvckFjY291bnRJZCI6MjQ0MywiZG9jdG9yUHJvZmlsZUlkIjo4ODkxLCJpc0FjdGl2ZSI6ZmFsc2UsInN0YXR1c1VwZGF0ZWRBdCI6MTc4NDEwNzAwMCwib2NjdXJyZWRBdCI6MTc4NDEwNzAwMCwic2NoZW1hVmVyc2lvbiI6MywicHJvZmlsZVZlcnNpb24iOjJ9",
    "attributes": {
      "eventType": "DoctorProfileStatusUpdated",
      "eventId": "22222222-2222-4222-8222-222222222222",
      "schemaVersion": "3"
    },
    "messageId": "12345678901234568",
    "orderingKey": "2443",
    "publishTime": "2026-07-16T03:56:40Z"
  }
}
```

## Consumer implementation checklist

- Create and consume from a subscription owned by your service. Do not pull
  from another service's subscription.
- Decode `message.data` as Base64 when using REST/JSON output, then parse the
  resulting bytes as UTF-8 JSON.
- Cross-check attribute `eventType`, payload `__type`, and `schemaVersion`.
- Persist `eventId` as an idempotency key and make duplicate processing a
  no-op.
- Key the doctor projection by `doctorId`; retain `doctorAccountId` and
  `doctorProfileId` when needed for integration lookup.
- Apply an event only when `profileVersion` is newer than the last version
  durably applied for that doctor.
- For `DoctorProfileStatusUpdated`, handle both `true` and `false` rather than
  treating the event as deactivation-only.
- Ignore unknown additive fields while rejecting unsupported event types or
  schema versions according to the consumer's retry/dead-letter policy.
- Commit the business result and idempotency/version state before
  acknowledging the Pub/Sub message.

To inspect a pulled message with `jq`:

```bash
gcloud pubsub subscriptions pull "$SUB" \
  "--project=$PROJECT_ID" \
  "--limit=1" \
  "--no-auto-ack" \
  "--format=json" |
jq '.[] | {
  messageId: .message.messageId,
  attributes: .message.attributes,
  orderingKey: .message.orderingKey,
  publishTime: .message.publishTime,
  data: (.message.data | @base64d | fromjson)
}'
```
