use crate::config::{FieldConfig, FieldType};
use crate::data::presets::PresetEntry;
use std::collections::HashMap;

/// A node in the preset hierarchy.
#[derive(Debug, Clone)]
pub enum TreeNode {
    Branch {
        axis_value: String,
        children: Vec<TreeNode>,
        count: usize,
    },
    Leaf {
        preset_name: String,
        description: Option<String>,
    },
}

/// The root of a built preset tree for one module.
#[derive(Debug, Clone)]
pub struct PresetTree {
    /// Axis-drilled branches (and their nested children), sorted alphabetically.
    pub roots: Vec<TreeNode>,
    /// Presets that had an empty/missing value for at least one axis.
    pub ungrouped: Vec<TreeNode>,
    /// `roots` plus a synthetic "Ungrouped (N)" branch appended when `ungrouped` is non-empty.
    /// Used by the picker's `current_nodes` so the root display is a single contiguous slice.
    /// Kept in sync with `roots` and `ungrouped` at build time.
    pub roots_with_ungrouped: Vec<TreeNode>,
}

/// Reasons a `preset_axes` entry is invalid.
#[derive(Debug, PartialEq)]
pub enum AxisError {
    UnknownField(String),
    CompositeArrayField(String),
    PresetExcludedField(String),
}

/// Build a `PresetTree` from a flat list of presets and an ordered axis list.
///
/// Branches are sorted alphabetically; leaves preserve the saved order.
/// A preset whose axis value is absent or empty lands in `ungrouped`.
pub fn build(presets: &[PresetEntry], axes: &[String]) -> PresetTree {
    if axes.is_empty() {
        let ungrouped: Vec<TreeNode> = presets
            .iter()
            .map(|p| TreeNode::Leaf {
                preset_name: p.name.clone(),
                description: p.description.clone(),
            })
            .collect();
        // No real roots; all presets are ungrouped. Synthesize the ungrouped branch.
        let roots_with_ungrouped = make_roots_with_ungrouped(&[], &ungrouped);
        return PresetTree {
            roots: Vec::new(),
            ungrouped,
            roots_with_ungrouped,
        };
    }

    let mut roots: Vec<TreeNode> = Vec::new();
    let mut ungrouped: Vec<TreeNode> = Vec::new();

    for preset in presets {
        // Check if any axis value is missing or empty — if so, this preset is ungrouped.
        let has_all = axes
            .iter()
            .all(|a| preset.values.get(a).map(|v| !v.is_empty()).unwrap_or(false));

        if !has_all {
            ungrouped.push(TreeNode::Leaf {
                preset_name: preset.name.clone(),
                description: preset.description.clone(),
            });
            continue;
        }

        insert_at_level(&mut roots, preset, axes, 0);
    }

    sort_branches(&mut roots);

    let roots_with_ungrouped = make_roots_with_ungrouped(&roots, &ungrouped);
    PresetTree {
        roots,
        ungrouped,
        roots_with_ungrouped,
    }
}

/// Build the combined root list: sorted branches followed by the synthetic
/// "Ungrouped (N)" branch when `ungrouped` is non-empty.
fn make_roots_with_ungrouped(roots: &[TreeNode], ungrouped: &[TreeNode]) -> Vec<TreeNode> {
    let mut combined: Vec<TreeNode> = roots.to_vec();
    if !ungrouped.is_empty() {
        combined.push(TreeNode::Branch {
            axis_value: format!("Ungrouped ({})", ungrouped.len()),
            children: ungrouped.to_vec(),
            count: ungrouped.len(),
        });
    }
    combined
}

fn insert_at_level(nodes: &mut Vec<TreeNode>, preset: &PresetEntry, axes: &[String], depth: usize) {
    if depth >= axes.len() {
        nodes.push(TreeNode::Leaf {
            preset_name: preset.name.clone(),
            description: preset.description.clone(),
        });
        return;
    }

    let axis = &axes[depth];
    let value = preset.values.get(axis).cloned().unwrap_or_default();

    // Find or create a matching Branch at this level.
    let branch_pos = nodes.iter().position(|n| match n {
        TreeNode::Branch { axis_value, .. } => axis_value == &value,
        TreeNode::Leaf { .. } => false,
    });

    if let Some(pos) = branch_pos {
        if let TreeNode::Branch {
            children, count, ..
        } = &mut nodes[pos]
        {
            *count += 1;
            insert_at_level(children, preset, axes, depth + 1);
        }
    } else {
        let mut children = Vec::new();
        insert_at_level(&mut children, preset, axes, depth + 1);
        nodes.push(TreeNode::Branch {
            axis_value: value,
            children,
            count: 1,
        });
    }
}

fn sort_branches(nodes: &mut [TreeNode]) {
    nodes.sort_by(|a, b| match (a, b) {
        (TreeNode::Branch { axis_value: av, .. }, TreeNode::Branch { axis_value: bv, .. }) => {
            av.cmp(bv)
        }
        // Branches before leaves at any given level.
        (TreeNode::Branch { .. }, TreeNode::Leaf { .. }) => std::cmp::Ordering::Less,
        (TreeNode::Leaf { .. }, TreeNode::Branch { .. }) => std::cmp::Ordering::Greater,
        (TreeNode::Leaf { .. }, TreeNode::Leaf { .. }) => std::cmp::Ordering::Equal,
    });
    for node in nodes.iter_mut() {
        if let TreeNode::Branch { children, .. } = node {
            sort_branches(children);
        }
    }
}

/// Validate that each axis name in `axes` refers to a field that:
/// - exists in `fields`
/// - is not `CompositeArray`
/// - does not have `preset_exclude = true`
///
/// Returns `Ok(())` if all axes are valid, or `Err(Vec<AxisError>)` listing
/// every violation found.
pub fn validate_axes(axes: &[String], fields: &[FieldConfig]) -> Result<(), Vec<AxisError>> {
    let field_map: HashMap<&str, &FieldConfig> =
        fields.iter().map(|f| (f.name.as_str(), f)).collect();

    let errors: Vec<AxisError> = axes
        .iter()
        .filter_map(|axis| match field_map.get(axis.as_str()) {
            None => Some(AxisError::UnknownField(axis.clone())),
            Some(f) if f.field_type == FieldType::CompositeArray => {
                Some(AxisError::CompositeArrayField(axis.clone()))
            }
            Some(f) if f.preset_exclude == Some(true) => {
                Some(AxisError::PresetExcludedField(axis.clone()))
            }
            Some(_) => None,
        })
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Build a suggested preset name from axis values, joining non-empty values with " · ".
///
/// Example: `values = {method: "V60", bean: "Onyx"}`, `axes = ["method", "bean"]`
/// → `"V60 · Onyx"`.
pub fn suggest_preset_name(values: &HashMap<String, String>, axes: &[String]) -> String {
    axes.iter()
        .filter_map(|a| {
            let v = values.get(a)?;
            if v.is_empty() { None } else { Some(v.as_str()) }
        })
        .collect::<Vec<_>>()
        .join(" \u{00B7} ")
}
