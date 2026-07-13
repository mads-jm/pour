pub(super) mod composite;
pub(super) mod fields;
pub(super) mod panel;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;
use crate::priors::ResolvedPanel;
use crate::visibility::visible_field_indices;

/// Minimum form height (rows) preserved before a stacked priors panel is given
/// any space. Below this, the panel is not rendered stacked (Architect #3).
const MIN_FORM_HEIGHT: u16 = 9;

/// Split the full form area into (form_area, optional priors render).
///
/// - No panel or width unavailable → full area to the form.
/// - Wide terminal → panel on the right ([`panel::PANEL_WIDTH`]).
/// - Narrow terminal → panel stacked below, collapsing to a one-line hint when
///   the height budget is too small (Architect decision #3).
///
/// Returns the form area plus an optional `(Rect, collapsed)` describing where
/// and how to paint the panel after the form renders.
fn split_for_panel(
    area: Rect,
    priors: Option<&ResolvedPanel>,
    collapsed: bool,
) -> (Rect, Option<(Rect, bool, &ResolvedPanel)>) {
    let Some(panel) = priors else {
        return (area, None);
    };

    // Collapsed by user (Ctrl+R): reserve a single hint row at the bottom.
    if collapsed {
        if area.height <= MIN_FORM_HEIGHT {
            return (area, None);
        }
        let form = Rect {
            height: area.height - 1,
            ..area
        };
        let hint = Rect {
            y: area.y + area.height - 1,
            height: 1,
            ..area
        };
        return (form, Some((hint, true, panel)));
    }

    // Side-by-side when the terminal is wide enough.
    if area.width >= panel::SIDE_BY_SIDE_MIN_WIDTH {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(panel::PANEL_WIDTH)])
            .split(area);
        return (chunks[0], Some((chunks[1], false, panel)));
    }

    // Stacked below: give the panel what's left after the form minimum, up to a
    // reasonable cap. Collapse to a one-line hint if the budget is too small.
    let available = area.height.saturating_sub(MIN_FORM_HEIGHT);
    if available < panel::STACKED_MIN_ROWS {
        // Not enough room for a box — collapse to a single hint row (if we can
        // spare even one row without clipping the form).
        if area.height <= MIN_FORM_HEIGHT {
            return (area, None);
        }
        let form = Rect {
            height: area.height - 1,
            ..area
        };
        let hint = Rect {
            y: area.y + area.height - 1,
            height: 1,
            ..area
        };
        return (form, Some((hint, true, panel)));
    }

    // Cap the stacked panel so it never dominates the screen.
    let panel_h = available
        .min(panel.rows.len() as u16 + 5)
        .max(panel::STACKED_MIN_ROWS);
    let form = Rect {
        height: area.height - panel_h,
        ..area
    };
    let panel_area = Rect {
        y: area.y + form.height,
        height: panel_h,
        ..area
    };
    (form, Some((panel_area, false, panel)))
}

/// Render the form view for the currently selected module.
pub fn render(app: &App, frame: &mut Frame) {
    let module_key = match app.module_keys.get(app.selected_module) {
        Some(k) => k,
        None => return,
    };
    let module = match app.config.modules.get(module_key) {
        Some(m) => m,
        None => return,
    };
    let form_state = match &app.form_state {
        Some(fs) => fs,
        None => return,
    };

    let full_area = frame.area();

    // Carve out the read-only priors panel (right / stacked-below / collapsed).
    // Never rendered while an overlay is active, to avoid visual clutter behind
    // the modal.
    let overlay_active = form_state.preset_picker.is_some()
        || form_state.preset_overlay.is_some()
        || form_state.callout_title_edit.is_some()
        || form_state.sub_form.is_some()
        || form_state.composite_open
        || form_state.textarea_open
        || form_state.dropdown_open;

    let (area, panel_render) = if overlay_active {
        (full_area, None)
    } else {
        split_for_panel(
            full_area,
            form_state.priors_panel.as_ref(),
            form_state.priors_collapsed,
        )
    };

    // Layout: title bar, field list, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Min(1),    // fields
            Constraint::Length(3), // footer / validation
        ])
        .split(area);

    // Title: "pour <key> — Display Name" to reinforce the CLI command
    let display_name = module
        .display_name
        .as_deref()
        .unwrap_or(module_key.as_str());
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" ▽ pour {module_key}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" — {display_name} "),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // has_picker is false when axes are empty OR axes failed validation (axis_warnings non-empty).
    let has_picker = !module.preset_axes.is_empty() && form_state.axis_warnings.is_empty();
    fields::render_fields(frame, chunks[1], &module.fields, form_state, has_picker);

    // Footer: validation errors, axis warnings, delete confirmation, or key hints
    let footer_content = if !form_state.validation_errors.is_empty() {
        let error_text = form_state.validation_errors.join("; ");
        Line::from(Span::styled(
            format!(" Error: {error_text}"),
            Style::default().fg(Color::Red),
        ))
    } else if !form_state.axis_warnings.is_empty() {
        let warn_text = form_state.axis_warnings.join(", ");
        Line::from(vec![
            Span::styled(
                format!(" \u{26A0} preset_axes invalid: {warn_text}"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" — picker disabled", Style::default().fg(Color::DarkGray)),
        ])
    } else if form_state.confirm_delete_preset {
        let name = form_state.selected_preset_name.clone().unwrap_or_default();
        Line::from(vec![
            Span::styled(
                format!(" Delete \"{name}\"?"),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" (y/n)", Style::default().fg(Color::Yellow)),
        ])
    } else {
        // Contextual hints: "t title" appears when focused on a textarea row
        // that has an active callout and the editor is closed.
        let visible = visible_field_indices(&module.fields, &form_state.field_values);
        let active_field_cfg =
            if form_state.active_field >= 1 && form_state.active_field <= visible.len() {
                visible
                    .get(form_state.active_field - 1)
                    .and_then(|&ci| module.fields.get(ci))
            } else {
                None
            };
        let show_title_hint = !form_state.textarea_open
            && active_field_cfg.is_some_and(|f| {
                f.field_type == crate::config::FieldType::Textarea
                    && (form_state.callout_overrides.contains_key(&f.name)
                        || f.callout.is_some()
                        || form_state.callout_overrides.contains_key("_callout_type"))
            });

        let mut spans = vec![
            Span::styled(" s", Style::default().fg(Color::Yellow)),
            Span::raw(" save  "),
            Span::styled("d", Style::default().fg(Color::Yellow)),
            Span::raw(" delete  "),
        ];
        if has_picker {
            spans.push(Span::styled("p", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" picker  "));
        } else {
            spans.push(Span::styled("←→", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" cycle  "));
        }
        spans.extend([
            Span::styled("↑↓/Tab", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" interact  "),
        ]);
        if show_title_hint {
            spans.push(Span::styled("t", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" title  "));
        }
        spans.push(Span::styled("Esc", Style::default().fg(Color::Yellow)));
        spans.push(Span::raw(" clear/back"));
        Line::from(spans)
    };
    let footer = Paragraph::new(footer_content).block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, chunks[2]);

    // Read-only priors panel: right, stacked-below, or a one-line hint.
    if let Some((panel_area, collapsed, panel)) = panel_render {
        if collapsed {
            panel::render_collapsed(frame, panel_area, panel);
        } else {
            panel::render_panel(frame, panel_area, panel);
        }
    }

    // Preset picker overlay: drilldown tree modal
    if let Some(ref picker) = form_state.preset_picker {
        super::overlays::preset_picker::render(frame, area, picker, &module.preset_axes);
    }

    // Preset save overlay renders before sub-form overlay
    if let Some(ref overlay) = form_state.preset_overlay {
        super::overlays::small::render_preset_save(frame, area, overlay);
    }

    // Callout-title edit overlay.
    if let Some(ref edit) = form_state.callout_title_edit {
        super::overlays::small::render_callout_title(frame, area, edit);
    }

    // Sub-form overlay renders LAST so it paints over footer and fields
    if let Some(sub_form) = &form_state.sub_form
        && let Some(templates) = &app.config.templates
        && let Some(template) = templates.get(&sub_form.template_name)
    {
        super::overlays::sub_form::render(frame, area, sub_form, template);
    }
}
