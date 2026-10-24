//! CI canary integration test. Cargo test is expected to fail this case.

#[test]
fn canary_integration_fails() {
    assert_eq!(1, 2, "ci canary integration test");
}
