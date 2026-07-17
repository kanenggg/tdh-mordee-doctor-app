# tdh-protocol

Central protobuf/gRPC protocol definitions for TDH (Telehealth Doctor Hub) services. This repository provides type-safe, cross-language contracts for Scala, Java, Rust, Python, and TypeScript services.

## Overview

- **Single Source of Truth**: Proto files define all shared types
- **Type Safety**: Sealed traits/enum/discriminated unions across languages
- **Cross-Language**: All services use identical types with compile-time safety
- **Backward Compatible**: Protobuf ensures field number safety

## Repository Structure

```
tdh-protocol/
├── protos/                        # Proto definitions
│   ├── common/                    # Shared types
│   │   ├── consulting.proto       # ConsultationChannel, BookingType
│   │   ├── patient.proto          # PatientIdentity
│   │   ├── localization.proto     # Localized<T> equivalent
│   │   └── events.proto           # ConsultationEvent (oneof)
│   ├── onboarding/
│   │   └── doctor.proto           # OnBoardingInfo, OnBoardingStatus
│   ├── appointment/
│   │   └── appointment.proto      # AppointmentStatusDoc
│   └── notification/
│       └── notification.proto     # NotificationPayload, ScheduledNotificationTask
├── buf.yaml                       # Buf schema registry config
├── buf.gen.yaml                   # Code generation config
├── Makefile                       # Build commands
├── scala/                         # Scala generated code
├── rust/                          # Rust generated code
├── python/                        # Python generated code
└── typescript/                    # TypeScript generated code
```

## Quick Start

### Prerequisites

```bash
# Install Buf
brew install bufbuild/buf/buf

# Or download from https://docs.buf.build/installation

# For GCP Artifact Registry publishing
gcloud auth configure-docker asia-southeast1-maven.pkg.dev
gcloud auth configure-docker asia-southeast1-python.pkg.dev
gcloud auth configure-docker asia-southeast1-npm.pkg.dev
```

### Generate Code

```bash
# Generate code for all languages
make gen
# or: buf generate
```

### Verify Protos

```bash
# Lint proto files
make lint
# or: buf lint

# Build proto files
make build
# or: buf build

# Check for breaking changes
make break
# or: buf breaking --against '.git#branch=main'
```

### Run Tests

```bash
# Test all languages
make test-all

# Test specific language
make test-rust
make test-scala
make test-python
make test-typescript
```

## Language-Specific Usage

### Scala (ScalaPB)

**In `build.sbt`:**
```scala
libraryDependencies += "tdh" %% "tdh-protocol" % "0.1.0"

import tdh.protocol.common._
import tdh.protocol.onboarding._

// Usage with sealed traits (exhaustive matching)
val event = ConsultationEvent(
  event = ConsultationEvent.Event.TimeslotReserved(
    TimeslotReserved(
      bookingId = "booking-123",
      doctorId = 2443,
      timestamp = Some(Instant.now())
    )
  )
)

event.event match {
  case ConsultationEvent.Event.TimeslotReserved(e) => handle(e)
  case ConsultationEvent.Event.SessionCreated(e) => handle(e)
  // Compile-time exhaustiveness check!
}
```

### Rust (tonic + prost)

**In `Cargo.toml`:**
```toml
[dependencies]
tdh-protocol = { version = "0.1.0", registry = "tdh-protocol" }
```

**Usage:**
```rust
use tdh_protocol::common::{ConsultationEvent, consultation_event::Event};

// Create event
let event = ConsultationEvent {
    event: Some(Event::TimeslotReserved(TimeslotReserved {
        booking_id: "booking-123".to_string(),
        doctor_id: 2443,
        timestamp: Some(timestamp_now()),
    }))
};

// Exhaustive match (compile-time enforced)
match event.event {
    Some(Event::TimeslotReserved(e)) => handle(e),
    Some(Event::SessionCreated(e)) => handle(e),
    None => handle_unknown(),
}
```

### Python (betterproto)

**Install:**
```bash
pip install tdh-protocol \
  --extra-index-url https://asia-southeast1-python.pkg.dev/tdh-project/tdh-python/simple/
```

**Usage:**
```python
from tdh_protocol.common import ConsultationEvent, TimeslotReserved

# Create event
event = ConsultationEvent()
event.timeslot_reserved.booking_id = "booking-123"
event.timeslot_reserved.doctor_id = 2443

# Serialize
bytes_data = event.SerializeToString()

# Deserialize
decoded = ConsultationEvent()
decoded.ParseFromString(bytes_data)

# Type-safe access
if decoded.timeslot_reserved is not None:
    print(f"Booking: {decoded.timeslot_reserved.booking_id}")
```

### TypeScript (protobuf-ts)

**Install:**
```bash
npm config set @tdh:registry https://asia-southeast1-npm.pkg.dev/tdh-project/tdh-npm/
npm install @tdh/protocol
```

**Usage:**
```typescript
import { ConsultationEvent } from '@tdh/protocol';

// Create event (discriminated union)
const event: ConsultationEvent = {
  event: {
    case: 'timeslotReserved',
    value: {
      bookingId: 'booking-123',
      doctorId: 2443,
      timestamp: Timestamp.now()
    }
  }
};

// Exhaustive pattern matching
switch (event.event.case) {
  case 'timeslotReserved':
    handle(event.event.value);
    break;
  case 'sessionCreated':
    handle(event.event.value);
    break;
  // TypeScript error if case missing!
}
```

## Publishing

### Publish to GCP Artifact Registry

```bash
# Publish all languages
make publish-all

# Publish specific language
make publish-scala
make publish-rust
make publish-python
make publish-typescript
```

### List Published Packages

```bash
make list-packages
```

## Proto Definitions

### Common Types

**ConsultationChannel** - Channel type for consultations:
```protobuf
enum ConsultationChannel {
  CONSULTATION_CHANNEL_UNSPECIFIED = 0;
  VIDEO = 1;
  CHAT = 2;
  VOICE = 3;
}
```

**BookingType** - Appointment booking type:
```protobuf
enum BookingType {
  BOOKING_TYPE_UNSPECIFIED = 0;
  SCHEDULED = 1;
  INSTANT = 2;
}
```

**PatientIdentity** - Patient identifier:
```protobuf
message PatientIdentity {
  int32 account_id = 1;
  int32 user_profile_id = 2;
  int32 tenant_id = 3;
  string oidc_user_id = 4;
}
```

**ConsultationEvent** - Domain events (discriminated union):
```protobuf
message ConsultationEvent {
  oneof event {
    TimeslotReserved timeslot_reserved = 1;
    SessionCreated session_created = 2;
    DoctorJoined doctor_joined = 3;
    PatientJoined patient_joined = 4;
  }
}
```

### Versioning

This project uses semantic versioning:

- **Major version** (v1, v2): Breaking changes - new proto package names
- **Minor version** (v1.1, v1.2): Non-breaking additions - new fields/messages
- **Patch version** (v1.1.1): Bug fixes - no proto changes

## Backward Compatibility

### ✅ SAFE (Non-Breaking) Changes

- Add new field
- Add new message
- Add new enum value
- Add new oneof variant
- Deprecate field

### ❌ BREAKING Changes (Require Major Version)

- Remove field
- Change field number
- Change field type
- Remove enum value
- Rename package/message

## GCP Artifact Registry Setup

### Create Repositories

```bash
# Enable Artifact Registry API
gcloud services enable artifactregistry.googleapis.com

# Scala/Java (Maven)
gcloud artifacts repositories create tdh-maven \
    --repository-format=maven \
    --location=asia-southeast1

# Rust (Cargo - raw format)
gcloud artifacts repositories create tdh-cargo \
    --repository-format=raw \
    --location=asia-southeast1

# Python (PyPI)
gcloud artifacts repositories create tdh-python \
    --repository-format=python \
    --location=asia-southeast1

# TypeScript/JavaScript (npm)
gcloud artifacts repositories create tdh-npm \
    --repository-format=npm \
    --location=asia-southeast1
```

## Contributing

1. **Add proto files** to `protos/` directory
2. **Run `make gen`** to generate code
3. **Run `make test-all`** to verify
4. **Run `make lint`** to check style
5. **Run `make break`** to check breaking changes
6. **Submit PR** for review

## License

Internal TDH project - All rights reserved
