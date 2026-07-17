// Doctor identity definitions for HTTP integration tests.
// These are loaded from http-client.env.json as environment variables.
//
// Available identities:
//   {{doctorIdentity}}       - default test doctor (accountId: 321, accountType: 2)
//   {{otherDoctorIdentity}}  - different doctor for unauthorized tests (accountId: 999)
//   {{patientIdentity}}      - patient identity (accountId: 123, accountType: 1)
//
// Usage in .http files:
//   {{userIdentityHeader}}: {{doctorIdentity}}
//
// Host:
//   {{host}} - base URL from env (default: http://localhost:8085)
