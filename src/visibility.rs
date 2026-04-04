use std::collections::HashMap;

use crate::config::FieldConfig;

/// Returns whether a single field should be visible given current form values.
///
/// - Fields with `show_when: None` are always visible.
/// - `equals` variant: visible if `field_values[show_when.field] == equals_value`.
/// - `one_of` variant: visible if `field_values[show_when.field]` matches any value in the list.
/// - If the referenced field is absent from `field_values` or is an empty string, the field is
///   hidden.
/// - All comparisons are case-sensitive.
pub fn is_field_visible(field: &FieldConfig, field_values: &HashMap<String, String>) -> bool {
    let Some(ref sw) = field.show_when else {
        return true;
    };

    let current = match field_values.get(&sw.field) {
        Some(v) if !v.is_empty() => v.as_str(),
        _ => return false,
    };

    if let Some(ref expected) = sw.equals {
        return current == expected.as_str();
    }

    if let Some(ref list) = sw.one_of {
        return list.iter().any(|v| v.as_str() == current);
    }

    // show_when present but neither equals nor one_of set — config validation should catch this,
    // but we default to hidden rather than accidentally showing the field.
    false
}

/// Returns the indices (into the original `fields` slice) of all currently visible fields.
///
/// Iterates `fields` in order and collects the index of each field for which
/// `is_field_visible` returns `true`.
pub fn visible_field_indices(
    fields: &[FieldConfig],
    field_values: &HashMap<String, String>,
) -> Vec<usize> {
    fields
        .iter()
        .enumerate()
        .filter_map(|(i, f)| {
            if is_field_visible(f, field_values) {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}
