//! Static guardrails for appointment upstream sensitive logging.
//!
//! Past-visit upstream payloads can contain medical history. Clients must not log
//! raw response bodies when parsing or handling upstream errors.

#[test]
fn appointment_upstream_clients_do_not_log_full_payloads() {
    let files = ["src/module/appointment/external/qolphin_client.rs"];

    let forbidden_patterns = [
        "response_body = %body",
        "response_body = %response_body",
        "response_body = %",
    ];

    for file in files {
        let source = std::fs::read_to_string(file).expect("read source file");
        for pattern in forbidden_patterns {
            assert!(
                !source.contains(pattern),
                "{file} must not contain sensitive logging pattern {pattern:?}"
            );
        }
    }
}
