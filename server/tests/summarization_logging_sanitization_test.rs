//! Static guardrails for consultation summarization sensitive logging.
//!
//! Summary notes, prescriptions, allergy text, and upstream response bodies can
//! contain medical PHI/PII. These clients must log only safe metadata.

#[test]
fn summarization_upstream_clients_do_not_log_full_payloads() {
    let files = [
        "src/module/consultation/summarization/external/biz_apm_http_client.rs",
        "src/module/consultation/summarization/external/jade_http_client.rs",
    ];

    let forbidden_patterns = [
        "request_body_json",
        "body = %response_text",
        "error = %response_text",
        "status, response_text",
        "request body",
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
