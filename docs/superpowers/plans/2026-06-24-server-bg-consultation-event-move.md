# Server BG Consultation Event Move Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move consultation-event handling ownership into the `server-bg` workspace member.

**Architecture:** `server-bg` owns the Pub/Sub push HTTP route and its own consultation event handler/repo/state-machine code. The existing `server` crate remains available for shared infrastructure during the transition, but `server-bg` must not delegate consultation-event processing to `server::module::consultation::ConsultationEventHandler`.

**Tech Stack:** Rust, Axum, Firebase RTDB REST repository, GCP Pub/Sub push envelope, existing TDH protocol models.

---

### Task 1: Add Server-BG Consultation Event Module

**Files:**
- Create: `server-bg/src/consultation_event/mod.rs`
- Create: `server-bg/src/consultation_event/event_handler.rs`
- Create: `server-bg/src/consultation_event/models.rs`
- Create: `server-bg/src/consultation_event/repository.rs`
- Create: `server-bg/src/consultation_event/state_machine.rs`
- Create: `server-bg/src/consultation_event/notification_publisher.rs`
- Create: `server-bg/src/consultation_event/notification_templates.rs`

- [ ] Copy the current consultation-event-specific implementation from `server/src/module/consultation/`.
- [ ] Rewrite imports so server-bg owns these modules and only imports shared infrastructure from `server::core`, `server::repo`, `server::module::notification`, `server::module::patient`, and `server::module::webhook`.
- [ ] Keep domain/state idempotency fields in the RTDB appointment model.

### Task 2: Wire Server-BG To Its Own Handler

**Files:**
- Modify: `server-bg/src/lib.rs`
- Modify: `server-bg/src/main.rs`

- [ ] Add a route-level trait for testable event handling.
- [ ] Make the HTTP route depend on `Arc<dyn ConsultationStatusChangedHandler>`.
- [ ] Construct `server_bg::consultation_event::ConsultationEventHandler` in `server-bg/src/main.rs`.
- [ ] Remove use of `server::module::consultation::ConsultationEventHandler` from `server-bg`.

### Task 3: Keep Server Runtime Out Of Consultation Event Consumption

**Files:**
- Modify: `server/src/bootstrap.rs`

- [ ] Keep the consultation pull subscriber disabled in `server`.
- [ ] Do not remove the old server consultation module in this change.

### Task 4: Verify

**Commands:**
- `cargo fmt`
- `cargo test -p server-bg`
- `cargo check -p server-bg`
- `cargo check -p server`
