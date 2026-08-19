use dioxus::prelude::*;
use dioxus_primitives::alert_dialog::{
    self, AlertDialogActionProps, AlertDialogActionsProps, AlertDialogCancelProps,
    AlertDialogDescriptionProps, AlertDialogTitleProps,
};

#[css_module("/src/components/primitives/alert_dialog/style.css")]
struct Styles;

/// Custom properties for our AlertDialog wrapper to allow layout overrides
#[derive(Props, Clone, PartialEq)]
pub struct AlertDialogProps {
    #[props(default)]
    pub id: String,
    #[props(default)]
    pub default_open: bool,
    pub open: bool,
    pub on_open_change: EventHandler<bool>,
    
    /// Pass-through for standard HTML attributes
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    
    /// Optional override for the maximum width (e.g. "60rem", "800px")
    #[props(default)]
    pub max_width: Option<String>,
    
    pub children: Element,
}

#[component]
pub fn AlertDialog(props: AlertDialogProps) -> Element {
    // Dynamically build an inline CSS string if max_width is provided
    let width_style = match props.max_width {
        Some(width) => format!("--dx-dialog-max-width: {};", width),
        None => String::new(),
    };
    rsx! {
        alert_dialog::AlertDialogRoot {
            class: Styles::dx_alert_dialog_backdrop,
            id: props.id,
            default_open: props.default_open,
            open: props.open,
            on_open_change: props.on_open_change,
            attributes: props.attributes,
            alert_dialog::AlertDialogContent {
                class: Styles::dx_alert_dialog.to_string(),
                // Apply the CSS variable directly to the content container
                style: "{width_style}",
                {props.children}
            }
        }
    }
}

#[component]
pub fn AlertDialogTitle(props: AlertDialogTitleProps) -> Element {
    rsx! {
        alert_dialog::AlertDialogTitle {
            class: Styles::dx_alert_dialog_title,
            attributes: props.attributes,
            {props.children}
        }
    }
}

#[component]
pub fn AlertDialogDescription(props: AlertDialogDescriptionProps) -> Element {
    rsx! {
        alert_dialog::AlertDialogDescription {
            class: Styles::dx_alert_dialog_description,
            attributes: props.attributes,
            {props.children}
        }
    }
}

#[component]
pub fn AlertDialogActions(props: AlertDialogActionsProps) -> Element {
    rsx! {
        alert_dialog::AlertDialogActions {
            class: Styles::dx_alert_dialog_actions,
            attributes: props.attributes,
            {props.children}
        }
    }
}

#[component]
pub fn AlertDialogCancel(props: AlertDialogCancelProps) -> Element {
    rsx! {
        alert_dialog::AlertDialogCancel {
            on_click: props.on_click,
            class: Styles::dx_alert_dialog_cancel,
            attributes: props.attributes,
            {props.children}
        }
    }
}

#[component]
pub fn AlertDialogAction(props: AlertDialogActionProps) -> Element {
    rsx! {
        alert_dialog::AlertDialogAction {
            class: Styles::dx_alert_dialog_action,
            on_click: props.on_click,
            attributes: props.attributes,
            {props.children}
        }
    }
}
