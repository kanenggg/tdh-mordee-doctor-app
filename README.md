# TDH Doctor App

Healthcare telemedicine API gateway for doctors built in Rust. This service manages appointments, consultations, notifications, doctor onboarding, and backoffice operations. It acts as an API gateway, proxying requests to external services including authentication, registration, EHR, and consultation platforms.

## Project Overview

The TDH Doctor App provides a comprehensive API interface for telemedicine services, supporting:

- **Appointment Management**: Schedule and manage doctor appointments
- **Consultation Handling**: Real-time video/audio consultations with state management
- **Doctor Onboarding**: Complete doctor profile setup with reference data validation
- **Notifications**: Firebase Cloud Messaging for real-time alerts
- **Patient Management**: Patient data retrieval and profile management
- **Backoffice Operations**: Admin functions for managing doctors and system operations
- **Reference Data**: Geographic and professional data (provinces, districts, hospitals, etc.)

## Architecture

- **Framework**: Axum web framework
- **Data Storage**: Google Firestore (primary), Firebase Realtime Database
- **Authentication**: Header-based (trusts upstream gateway)
- **External Services**: GCP Pub/Sub, Cloud Tasks, FCM
- **Documentation**: OpenAPI/Swagger
- **Testing**: Integration tests with axum-test

## Quick Start

### Prerequisites

- Rust 1.70+ (nightly recommended for workspace)
- Google Cloud SDK with authenticated credentials
- Docker (optional)

### Local Development Setup

1. **Install dependencies and authenticate with GCP**:
   ```bash
   gcloud auth application-default login
   ```

2. **Copy environment variables**:
   ```bash
   cp .env.example server/.env  # Copy and update with your values
   ```

3. **Build the project**:
   ```bash
   cargo build                      # Development build
   cargo build --release            # Release build
   ```

4. **Run the server**:
   ```bash
   cargo run                        # Run on 0.0.0.0:8080
   ```

### Development Utilities

```bash
cargo run --bin clear_test_notifications    # Clear test notifications (doctorId 2443)
cargo run --bin generate_test_notifications  # Generate test notifications
```

### Docker

```bash
docker build -t doctor-app .  # Build Docker image
docker run -p 8080:8080 doctor-app  # Run container
```

## API Documentation

### Swagger UI
- Access interactive API documentation at `http://localhost:8080/swagger`
- OpenAPI specification available at `http://localhost:8080/api-docs/openapi.json`

### API Endpoints

#### Health Check
- `GET /health` - Health check endpoint

#### Appointments (`/appointment/v1/`)
- Schedule, manage, and retrieve appointments

#### Consultations (`/consultation/v1/`)
- Manage consultation states and events

#### Doctor Onboarding (`/onboarding/v1/`)
- Doctor profile setup and document uploads

#### Patient Management (`/patient/v1/`)
- Patient data retrieval from registration service

#### User Profile (`/user/v1/`)
- Doctor profile management

#### Notifications (`/notifications/v1/`)
- Push notification management and delivery

#### Backoffice (`/backoffice/v1/`)
- Admin operations for system management

#### Reference Data (`/lookup/v1/`)
- Hierarchical lookup data:
  - Hospitals, universities, professions
  - Academic positions
  - Geographic data (provinces, districts, sub-districts, postal codes)

#### Webhooks (`/webhook/v1/`)
- Pub/Sub event handling
- Health check for webhook services

#### Task Scheduler (`/tasks/v1/`)
- Cloud Tasks callbacks for scheduled operations

## Configuration

### Environment Variables

| Environment Variable | Configuration Key | Description |
|---------------------|------------------|-------------|
| `SYS__HOST` / `SYS__PORT` | `sys.host` / `sys.port` | Server host and port |
| `SERVICE__AUTHN_SERVICE_BASE_URI` | `service.authn_service_base_uri` | Authentication service URL |
| `FIRESTORE__GCP_PROJECT_ID` | `firestore.gcp_project_id` | Firestore project ID |
| `FIREBASE__DATABASE_SECRET` | `firebase.database_secret` | Firebase RTDB auth secret |

### GCP Authentication

The service uses Application Default Credentials (ADC) for authentication with Google Cloud services. Set up authentication with:

```bash
gcloud auth application-default login
# OR
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json
```

## Development

### Running Tests

```bash
cargo test                          # Run all tests
cargo test webhook                  # Run tests matching "webhook"
cargo test --test webhook_test      # Run specific test file
cargo test test_name                # Run specific test function
cargo test -- --nocapture           # Run with output
```

### Linting and Formatting

```bash
cargo clippy                       # Run linter
cargo fmt                          # Format code
```

### Module Structure

The application follows a modular architecture in `server/src/module/`:

- Each module has its own router, handlers, and optionally repositories/services
- Repositories are injected via Axum `State(...)`
- Shared types are in `crates/tdh-common-models/`

### Adding a New Module

1. Create `server/src/module/<name>/` with `mod.rs`, `handlers.rs`, and optional `repo.rs` or `service.rs`
2. Define `pub fn router(...)` in `mod.rs` that returns `Router`
3. Add `pub mod <name>;` to `server/src/module/mod.rs`
4. Wire in `main.rs`: `.nest("/<name>/v1", module::<name>::router(...))`
5. Add handler paths and schemas to `server/src/openapi.rs`

## External Integrations

### Cloud Services

- **Google Cloud Firestore**: Primary data storage
- **Firebase Realtime Database**: Secondary storage and caching
- **Firebase Cloud Messaging**: Push notifications
- **Cloud Pub/Sub**: Event publishing
- **Cloud Tasks**: Scheduled job execution

### Service Integration

- **Authentication Service**: User identity validation
- **Registration Service**: Patient data retrieval
- **EHR System**: Electronic health records
- **Consultation Platform**: Video/audio call management

## Production Deployment

### Docker Deployment

```bash
docker build -t doctor-app .
docker-compose up -d
```

### Environment Setup for Production

- Set `NODE_ENV=production`
- Configure `FIREBASE__DATABASE_SECRET`
- Set up GCP service account credentials
- Configure external service URLs

### Kubernetes

Deployment manifests available in `k8s-deployment.yaml`:
- Configured for Traefik ingress
- Health checks enabled
- Resource limits and requests configured

## Error Handling

- Domain-level "not found" cases return HTTP 200 with typed JSON (e.g., `{ "__type": "AppointmentNotFound" }`)
- HTTP 4xx/5xx used for actual HTTP-level errors
- Structured error responses with consistent format

## Monitoring and Observability

- **GCP Cloud Logging**: Structured JSON logs with request correlation
- **OpenTelemetry**: Trace exports to OTLP collector
- **Health Checks**: `/health` endpoint for monitoring

## Contributing

1. Follow the module structure pattern
2. Add OpenAPI documentation for new endpoints
3. Write integration tests for new features
4. Update documentation when adding new functionality
5. Run tests and linters before committing

## License

Internal TDH project - proprietary code

## Support

For technical support or questions, contact the development team.