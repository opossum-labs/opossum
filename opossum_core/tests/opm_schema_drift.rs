use std::env;
use std::fs;
use std::path::PathBuf;

use opossum_core::opm_document::generate_opm_v1_schema_json;

/// Returns the path to `spec/v1/opm_v1.schema.json` relative to the workspace root.
fn schema_file_path() -> PathBuf {
    // `CARGO_MANIFEST_DIR` points to `<workspace>/opossum_core`
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("files_for_testing")
        .join("spec")
        .join("v1")
        .join("opm_v1.schema.json")
}

/// Normalizes CRLF (`\r\n`) to LF (`\n`) and trims trailing whitespace.
fn normalize_newlines(input: &str) -> String {
    input.replace("\r\n", "\n").trim().to_owned()
}

/// Formats a concise line-by-line diff showing only mismatched lines and nearby context.
fn format_line_diff(expected: &str, actual: &str) -> String {
    let exp_lines: Vec<&str> = expected.lines().collect();
    let act_lines: Vec<&str> = actual.lines().collect();
    let max_len = exp_lines.len().max(act_lines.len());

    let mut diff_output = String::new();
    let mut shown_hunks = 0;

    for i in 0..max_len {
        let exp = exp_lines.get(i).copied();
        let act = act_lines.get(i).copied();

        if exp != act {
            shown_hunks += 1;
            if shown_hunks > 15 {
                diff_output.push_str("\n... (further differences truncated)\n");
                break;
            }
            diff_output.push_str(&format!("\n--- Difference at line {} ---\n", i + 1));
            if i > 0 {
                if let Some(prev) = exp_lines.get(i - 1) {
                    diff_output.push_str(&format!("  {:>4} | {prev}\n", i));
                }
            }
            match exp {
                Some(line) => diff_output.push_str(&format!("- {:>4} | {line}\n", i + 1)),
                None => diff_output.push_str(&format!("- {:>4} | <EOF>\n", i + 1)),
            }
            match act {
                Some(line) => diff_output.push_str(&format!("+ {:>4} | {line}\n", i + 1)),
                None => diff_output.push_str(&format!("+ {:>4} | <EOF>\n", i + 1)),
            }
        }
    }
    diff_output
}

#[test]
fn verify_opm_v1_schema_up_to_date() {
    let generated_raw = generate_opm_v1_schema_json()
        .expect("Failed to serialize OpmDocument JSON schema");
    let schema_path = schema_file_path();

    if env::var("UPDATE_SCHEMA").as_deref() == Ok("1") {
        if let Some(parent) = schema_path.parent() {
            fs::create_dir_all(parent).expect("Failed to create spec/v1 directory");
        }
        let normalized = normalize_newlines(&generated_raw);
        fs::write(&schema_path, format!("{normalized}\n"))
            .expect("Failed to write updated schema file");
        return;
    }

    let committed_raw = fs::read_to_string(&schema_path).unwrap_or_else(|err| {
        panic!(
            "Could not read schema file at {}: {err}. \
             Run `UPDATE_SCHEMA=1 cargo test -p opossum_core --test opm_schema_drift` to generate it.",
            schema_path.display()
        )
    });

    let committed = normalize_newlines(&committed_raw);
    let generated = normalize_newlines(&generated_raw);

    if committed != generated {
        let diff = format_line_diff(&committed, &generated);
        panic!(
            "Schema drift detected in `{}`:\n{diff}\n\
             Run `UPDATE_SCHEMA=1 cargo test -p opossum_core --test opm_schema_drift` if this change is intentional.",
            schema_path.display()
        );
    }
}