use dioxus::prelude::*;

/// Controlled numeric text input that commits parsed `f64` values on blur or `Enter`,
/// formatting numbers with up to `precision` decimal places without trailing zeros.
#[component]
pub fn NumericCommitInput(
    value: Option<f64>,
    disabled: bool,
    #[props(default = 3)] precision: usize,
    #[props(default = false)] allow_empty: bool,
    on_commit: EventHandler<Option<f64>>,
) -> Element {
    let format_val = move |v: Option<f64>| {
        v.map_or_else(String::new, |num| {
            let formatted = format!("{num:.precision$}");
            if formatted.contains('.') {
                let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
                if trimmed.is_empty() || trimmed == "-" {
                    "0".to_string()
                } else {
                    trimmed.to_string()
                }
            } else {
                formatted
            }
        })
    };

    let val_memo = use_memo(use_reactive!(|value| value));
    let mut text = use_signal(|| format_val(value));

    use_effect(move || {
        let current = *val_memo.read();
        let formatted = format_val(current);
        if formatted != *text.peek() {
            text.set(formatted);
        }
    });

    let mut commit = move || {
        if disabled {
            return;
        }
        let raw = text.read().trim().to_string();
        if raw.is_empty() && allow_empty {
            text.set(String::new());
            on_commit.call(None);
        } else if let Ok(parsed) = raw.parse::<f64>()
            && parsed.is_finite()
        {
            text.set(format_val(Some(parsed)));
            on_commit.call(Some(parsed));
        } else {
            text.set(format_val(*val_memo.peek()));
        }
    };

    rsx! {
      input {
        r#type: "text",
        class: "form-control form-control-sm bg-black text-light border-secondary p-0 px-1 font-monospace text-end",
        style: "height: 22px; font-size: 11px;",
        disabled,
        value: "{text}",
        oninput: move |e: FormEvent| text.set(e.value()),
        onblur: move |_| commit(),
        onkeydown: move |e: KeyboardEvent| {
            if e.key() == Key::Enter {
                commit();
            }
        },
      }
    }
}
