# TDH Doctor App Codemaps

**Last Updated:** 2026-03-02

## Overview

This document provides architectural maps of the TDH Doctor App workspace. The
workspace includes the Rust doctor-app API gateway and the new Doctor Pool
service for patient-facing doctor ranking/search.

## Available Codemaps

### 1. [Frontend Codemap](./frontend.md)
- API routes and handlers structure
- Request/response patterns
- Authentication and middleware

### 2. [Backend/API Codemap](./backend.md)
- Core service architecture
- Data repositories and services
- External integrations

### 3. [Database Codemap](./database.md)
- Firestore collections and schemas
- Data relationships and models
- Reference data structure

### 4. [Integrations Codemap](./integrations.md)
- External service connections
- GCP services integration
- Event publishing and scheduling

### 5. [Workers/Codemap](./workers.md)
- Background job processing
- Cloud Tasks implementation
- Scheduled operations

## Architecture Overview

```
TDH Doctor App (Rust)
├── Workspace: [server, doctor-pool, crates/tdh-protocol/rust]
├── Framework: Axum
├── Storage: Firestore + Firebase RTDB
├── Auth: Header-based (upstream trusted)
├── Messaging: GCP Pub/Sub + FCM
└── Testing: axum-test integration
```

## Quick Navigation

- **Development**: See [README.md](../../README.md) for setup
- **API Documentation**: `/swagger` endpoint for interactive docs
- **Module Guidelines**: Refer to [CLAUDE.md](../../CLAUDE.md)
- **Doctor Pool**: See [docs/doctor-pool.md](../doctor-pool.md) for ranking,
  optional/multiple privilege filtering, and Redis production design

## Related Projects

- [Doctor Pool](../../doctor-pool/) - Patient-facing doctor ranking/search
- [TDH Protocol](../../crates/tdh-protocol/) - Shared protocol/domain types
- [OpenAPI Spec](../../server/src/openapi.rs) - API documentation definition
