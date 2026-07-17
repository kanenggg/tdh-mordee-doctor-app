# Test Data

This directory contains pre-encoded test data for cross-language compatibility testing.

## Files

- `rust_timeslot_reserved.bin` - ConsultationEvent encoded by Rust
- `scala_session_created.bin` - ConsultationEvent encoded by Scala
- `python_doctor_joined.bin` - ConsultationEvent encoded by Python
- `typescript_patient_identity.bin` - PatientIdentity encoded by TypeScript

## Generating Test Data

Run: `make test-data`

This will generate cross-language test data files to verify serialization compatibility between different language implementations.
