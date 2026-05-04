pub(super) mod preset_picker;
pub(super) mod small;
pub(super) mod sub_form;

use crate::data::preset_tree::TreeNode;

/// Build a breadcrumb string from the current picker path and axis labels.
pub(super) fn build_breadcrumb(picker: &crate::app::PresetPickerState, axes: &[String]) -> String {
    if picker.path.is_empty() {
        return axes
            .first()
            .cloned()
            .unwrap_or_else(|| "Preset".to_string());
    }
    let mut parts: Vec<String> = Vec::new();
    let mut nodes: &[TreeNode] = &picker.tree.roots_with_ungrouped;
    for &idx in picker.path.iter() {
        if let Some(TreeNode::Branch {
            axis_value,
            children,
            ..
        }) = nodes.get(idx)
        {
            parts.push(axis_value.clone());
            nodes = children;
        }
    }
    let next_axis = axes.get(picker.path.len()).cloned();
    let mut breadcrumb = parts.join(" \u{25B8} ");
    if let Some(ax) = next_axis {
        if !breadcrumb.is_empty() {
            breadcrumb.push_str(" \u{25B8} ");
        }
        breadcrumb.push_str(&ax);
    }
    breadcrumb
}

/// Return the currently-visible nodes for the picker's current depth.
pub(super) fn current_nodes(picker: &crate::app::PresetPickerState) -> &[TreeNode] {
    if picker.path.is_empty() {
        return &picker.tree.roots_with_ungrouped;
    }
    let mut nodes: &[TreeNode] = &picker.tree.roots_with_ungrouped;
    for &idx in &picker.path {
        if let Some(TreeNode::Branch { children, .. }) = nodes.get(idx) {
            nodes = children;
        } else {
            return &[];
        }
    }
    nodes
}
