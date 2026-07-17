# Test Data

This directory contains pre-encoded test data for cross-language compatibility testing.

## Files

- `rust_timeslot_reserved.bin` - ConsultationEvent encoded by Rust
- `scala_session_created.bin` - ConsultationEvent encoded by Scala
- `python_doctor_joined.bin` - ConsultationEvent encoded by Python
- `typescript_patient_identity.bin` - PatientIdentity encoded by TypeScript

## Generating Test Data

Run: `make test-data`

## Purpose

These files verify that:
1. Each language can correctly serialize protobuf messages
2. Other languages can correctly deserialize messages from different languages
3. Forward compatibility is maintained (old clients can read new data)

## Usage Example

```rust
// Rust test
let bytes = std::fs::read("test_data/python_doctor_joined.bin")?;
let event = ConsultationEvent::decode(&*bytes)?;
assert!(matches!(event.event, Some(consultation_event::Event::DoctorJoined(_))));
```
