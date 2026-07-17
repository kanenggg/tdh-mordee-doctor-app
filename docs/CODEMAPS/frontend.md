# Frontend Codemap (API Routes)

**Last Updated:** 2026-03-02
**Entry Points:** `/server/src/main.rs` - Router configuration

## Architecture

API gateway built with Axum framework, organized by domain modules with consistent routing patterns.

```mermaid
graph TD
    A[Server Router] --> B[Health & Docs]
    A --> C[Appointment Module]
    A --> D[Consultation Module]
    A --> E[Onboarding Module]
    A --> F[Patient Module]
    A --> G[User Module]
    A --> H[Notification Module]
    A --> I[Backoffice Module]
    A --> J[Lookup Module]
    A --> K[Webhook Module]
    A --> L[Task Scheduler]

    B --> B1[/health]
    B --> B2[/swagger]
    B --> B3[/api-docs/openapi.json]

    C --> C1[/appointment/v1/*]
    D --> D1[/consultation/v1/*]
    E --> E1[/onboarding/v1/*]
    F --> F1[/patient/v1/*]
    G --> G1[/user/v1/profile]
    H --> H1[/notifications/v1/*]
    I --> I1[/backoffice/v1/*]
    J --> J1[/lookup/v1/*]
    K --> K1[/webhook/v1/*]
    L --> L1[/tasks/v1/*]
```

## Key Modules

| Module | Purpose | Exports | Dependencies |
|--------|---------|---------|--------------|
| **appointment** | Schedule/manage appointments | Router | Firestore, Firebase |
| **consultation** | Handle consultations (states/events) | Router | Firestore, Pub/Sub |
| **onboarding** | Doctor profile setup | Router (shared) | Firestore, Upload |
| **patient** | Patient data retrieval | Router | HTTP Client |
| **user** | Doctor profile management | Router | Onboarding Repo |
| **notification** | Push notifications | Router (shared) | Firestore, FCM |
| **backoffice** | Admin operations | Router | Firestore |
| **lookup** | Reference data | Router | Firestore |
| **webhook** | Event handling | Router (shared) | Pub/Sub, Cloud Tasks |
| **tasks** | Scheduled jobs | Router | Cloud Services |

## API Route Structure

### Health & Documentation
```
GET  /health                           # Health check
GET  /swagger                         # Interactive API docs
GET  /api-docs/openapi.json           # OpenAPI spec
```

### Appointment Management (`/appointment/v1/`)
- Complete CRUD for doctor appointments
- Status management and filtering
- Integration with consultation system

### Consultation Management (`/consultation/v1/`)
- Consultation state machine (booking → started → completed → cancelled)
- Event handling and state transitions
- Integration with video/audio platforms

### Doctor Onboarding (`/onboarding/v1/`)
- Multi-step profile creation
- Document upload and verification
- Reference data validation
- **Shared repo with user module**

### Patient Management (`/patient/v1/`)
- Patient name retrieval from IAM gatekeeper
- Fallback handling for missing data
- HTTP client integration

### User Profile (`/user/v1/`)
- Doctor profile CRUD operations
- **Reuses onboarding repository**
- Profile image management

### Notifications (`/notifications/v1/`)
- Push notification delivery
- FCM token management
- **Shared with webhook module**

### Backoffice Operations (`/backoffice/v1/`)
- Admin functions for doctor management
- System administration
- Analytics and reporting

### Reference Data (`/lookup/v1/`)
- Hierarchical data:
  - Hospitals, universities, professions
  - Academic positions
  - Geographic: provinces → districts → sub-districts → postal codes
- Filtering and pagination support

### Webhooks (`/webhook/v1/`)
- Pub/Sub event publishing
- Health check endpoints
- **Shared with notification module**

### Task Scheduler (`/tasks/v1/`)
- Cloud Tasks callback endpoints
- Scheduled notification processing
- Delayed job execution

## Request/Response Patterns

### Authentication
All endpoints use header-based authentication:
- `tdh-sec-iam-user-identity` (JSON UserIdentity)
- Extractors: `DoctorIdentity` (canonical doctor `account_type == 2`; legacy `3` accepted by the extractor), `BackofficeIdentity` (`account_type == 4`), `PatientHeaders`

### Error Handling
- Domain errors: HTTP 200 with `__type` field
- HTTP errors: 4xx/5xx for protocol-level issues
- Consistent error response structure

### Pagination
- Cursor-based pagination where applicable
- `limit` and `offset` parameters
- Next cursor in response metadata

### Response Wrapping
```json
{
  "data": {...},        // Actual response data
  "metadata": {         // Optional metadata
    "cursor": "...",
    "total": 123
  }
}
```

## Middleware Stack

1. **GCP Logging Middleware**: Request correlation, sensitive data masking
2. **Authentication**: Header extraction and validation
3. **Error Handling**: Consistent error responses
4. **CORS**: Cross-origin request handling

## Development Patterns

### Handler Structure
```rust
// handlers.rs
pub async fn handle_request(
    State(state): State<AppState>,
    Path(params): Path<Params>,
    headers: Headers,
) -> AppResult<impl IntoResponse> {
    // Business logic here
    Ok(Json(response))
}
```

### Repository Pattern
```rust
// Generic repo traits
pub trait AppointmentRepo: Send + Sync {
    async fn create(&self, appointment: NewAppointment) -> AppResult<Appointment>;
    async fn find_by_doctor(&self, doctor_id: i32) -> AppResult<Vec<Appointment>>;
}

// Module-specific implementations
pub struct AppointmentRepoImpl {
    firestore: Arc<FirestoreRepo>,
}
```

## External Dependencies

### Required Services
- **IAM Gatekeeper**: User identity/profile lookup
- **EHR System**: Medical record integration
- **Consultation Platform**: Video/audio calls

### GCP Services
- **Firestore**: Primary data storage
- **Firebase RTDB**: Secondary storage
- **Pub/Sub**: Event publishing
- **Cloud Tasks**: Job scheduling
- **FCM**: Push notifications
