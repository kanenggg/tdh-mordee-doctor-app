# Star Gate pending doctor approvals API

Star Gate can read pending doctor approvals from the Doctor App backoffice onboarding API.

## Authentication

Both endpoints require the `tdh-sec-iam-user-identity` header containing a Mordee IAM user identity JSON payload with `accountType: 4` (backoffice account). Missing, malformed, or non-backoffice identities return HTTP `401`.

Example header value:

```json
{
  "accountId": 9001,
  "accountType": 4,
  "userProfileId": 456,
  "userMainProfileId": 456,
  "tenantId": 1
}
```

## List pending approvals

```http
GET /backoffice/v1/onboarding/pending?page=1&limit=20
```

Query parameters:

- `page` defaults to `1`.
- `limit` defaults to `20` and is clamped to `1..=100`.

Response:

```json
{
  "data": [
    {
      "doctorAccountId": 7001,
      "doctorProfileId": 9701,
      "firstName": { "th": "สมชาย", "en": "Somchai" },
      "lastName": { "th": "ใจดี", "en": "Jaidee" },
      "profession": [{ "locale": "en", "value": "General Practice" }],
      "academicPosition": [{ "locale": "en", "value": "Attending" }],
      "profileImageUrl": "https://cdn.example/profile.jpg",
      "status": "PendingApproval",
      "submittedAt": 1720000000
    }
  ],
  "page": 1,
  "limit": 20
}
```

The list endpoint only returns rows whose draft status is `PendingApproval`.

## Get pending approval detail

```http
GET /backoffice/v1/onboarding/pending/{doctorAccountId}
```

Success response:

```json
{
  "__type": "PendingDoctorApproval",
  "doctorAccountId": 7101,
  "doctorProfileId": 9801,
  "firstName": { "th": "สมชาย", "en": "Somchai" },
  "lastName": { "th": "ใจดี", "en": "Jaidee" },
  "profession": [{ "locale": "en", "value": "General Practice" }],
  "academicPosition": [{ "locale": "en", "value": "Attending" }],
  "licenseNumber": "LIC-123",
  "primaryMedicalSchool": [{ "locale": "en", "value": "Chula" }],
  "specialty": { "id": 10, "name": { "en": "Family Medicine", "th": "เวชศาสตร์ครอบครัว" } },
  "additionalSpecialties": [],
  "specialInterest": ["telehealth"],
  "address": {
    "addressDetail": "123 Safe St",
    "subDistrict": {},
    "district": {},
    "province": {},
    "postalCode": 10110
  },
  "workPlace": [],
  "additionalWorkplace": [],
  "profileImageUrl": "https://cdn.example/profile.jpg",
  "status": "PendingApproval",
  "submittedAt": 1720000000,
  "redactedFields": [
    "citizenId",
    "idCardImageUrl",
    "bookBankImageUrl",
    "medicalLicenseImageUrl",
    "educationLicenseImageUrl"
  ]
}
```

Unknown or non-pending profiles return HTTP `200` with a typed compatibility response:

```json
{ "__type": "PendingDoctorApprovalNotFound" }
```

## Safety contract

These endpoints select an explicit column allowlist from `doctor_profile_draft`. They do not serialize the full draft model, do not expose raw unknown profile fields, and do not return raw citizen ID or document image URLs. Sensitive fields are represented only by the `redactedFields` names in the detail response.
