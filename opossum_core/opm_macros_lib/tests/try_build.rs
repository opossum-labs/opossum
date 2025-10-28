#[test]
fn ensure_validated_macro_tests() {
    let t = trybuild::TestCases::new();

    // ✅ These should compile successfully
    t.pass("tests/pass/simple_struct.rs");
    t.pass("tests/pass/nested_struct.rs");
    t.pass("tests/pass/skip_field.rs");

    // ❌ These should fail with compile_error! emitted by the macro
    t.compile_fail("tests/fail/unvalidated_field.rs");
    t.compile_fail("tests/fail/bad_nested.rs");
    t.compile_fail("tests/fail/invalid_validate_struct.rs");
}
