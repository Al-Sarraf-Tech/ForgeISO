//! Form-field descriptor types used by the Configure tab UI.
//!
//! `FieldDef` carries label + display value + kind so the UI layer can render
//! consistently without knowing the underlying `App` field names. Re-exported
//! from [`crate::state`].

pub(crate) enum FieldKind {
    Text,
    Password,
    Toggle(bool),
}

pub(crate) struct FieldDef {
    pub(crate) label: &'static str,
    pub(crate) kind: FieldKind,
    pub(crate) value_str: String,
}

impl FieldDef {
    pub(crate) fn text(label: &'static str, value: &str) -> Self {
        Self {
            label,
            kind: FieldKind::Text,
            value_str: value.to_string(),
        }
    }

    pub(crate) fn password(label: &'static str, value: &str) -> Self {
        Self {
            label,
            kind: FieldKind::Password,
            value_str: value.to_string(),
        }
    }

    pub(crate) fn toggle(label: &'static str, value: bool) -> Self {
        Self {
            label,
            kind: FieldKind::Toggle(value),
            value_str: if value { "ON" } else { "OFF" }.into(),
        }
    }

    pub(crate) fn display_value(&self) -> String {
        match &self.kind {
            FieldKind::Password => {
                if self.value_str.is_empty() {
                    String::new()
                } else {
                    "*".repeat(self.value_str.len())
                }
            }
            _ => self.value_str.clone(),
        }
    }

    pub(crate) fn is_toggle(&self) -> bool {
        matches!(self.kind, FieldKind::Toggle(_))
    }
}
