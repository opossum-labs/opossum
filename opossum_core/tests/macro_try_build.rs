#[test]
fn ensure_validated_macro_tests() {
    let t = trybuild::TestCases::new();
    // These should compile successfully
    t.pass("tests/macro_ui/pass/simple_struct.rs");
    t.pass("tests/macro_ui/pass/nested_struct.rs");
    t.pass("tests/macro_ui/pass/skip_field.rs");
    // These should fail with compile_error! emitted by the macro
    t.compile_fail("tests/macro_ui/fail/unvalidated_field.rs");
    t.compile_fail("tests/macro_ui/fail/bad_nested.rs");
    t.compile_fail("tests/macro_ui/fail/invalid_validate_struct.rs");
}