use dioxus::prelude::*;

#[component]
pub fn DynamicListComponent(
    list_entries: Vec<Vec<String>>,
    delete_entry_handler: EventHandler<usize>,
    modify_entry_handler: EventHandler<usize>,
    edit_index: ReadSignal<Option<usize>>,
    readonly: bool,
) -> Element {
    let readonly = readonly || edit_index.read().is_some();
    rsx! {
        ul {
            class: "list-group border-start dynamic-list",
            id: "stackedAperturesList",
            for (entry_index , entry) in list_entries.iter().enumerate() {
                {
                    let class = if let Some(editing_index) = *edit_index.read()
                        && editing_index == entry_index
                    {
                        "d-flex list-group-item d-grid text-primary align-items-center border border-primary"
                    } else if entry_index % 2 == 0 {
                        "d-flex list-group-item d-grid text-secondary align-items-center"
                    } else {
                        "d-flex list-group-item d-grid text-secondary list-group-item-dark align-items-center"
                    };
                    rsx! {
                        li { class,
                            for value in entry.iter().cloned() {
                                span { class: "flex-grow-0", style: "width: {90/entry.len()}%;", {value} }
                            }
                            div { class: "ms-auto d-flex",
                                a {
                                    class: if readonly { "ms-auto text-muted" } else { "text-success ms-auto" },
                                    onclick: {
                                        move |_| {
                                            if !readonly {
                                                modify_entry_handler.call(entry_index);
                                            }
                                        }
                                    },
                                    role: if readonly { "" } else { "button" },
                                    "✎"
                                }
                                a {
                                    class: if readonly { "ms-auto text-muted" } else { "text-danger ms-auto" },
                                    onclick: {
                                        move |_| {
                                            if !readonly {
                                                delete_entry_handler.call(entry_index);
                                            }
                                        }
                                    },
                                    role: if readonly { "" } else { "button" },
                                    "🗑︎"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
