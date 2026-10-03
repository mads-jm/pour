pub(super) mod clip;
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

/// Where and how the priors panel goes this frame.
enum PanelRender<'a> {
    /// The column panel to the right of the form.
    Columns {
        area: Rect,
        panel: &'a ResolvedPanel,
        priors: usize,
        summary: bool,
    },
    /// A one-line hint on the bottom row: collapsed, too narrow, or empty.
    Hint { area: Rect, text: String },
}

/// Split the full form area into (form_area, optional priors render).
///
/// - Not resolved yet → full area to the form, nothing drawn.
/// - Empty state, `Ctrl+R` collapsed, or too narrow for one column → a
///   one-line hint on the bottom row.
/// - Otherwise → the column panel on the right, as wide as [`panel::fit`]
///   allows. The panel never moves below the form, so its rows stay aligned.
fn split_for_panel(
    area: Rect,
    priors: Option<&ResolvedPanel>,
    collapsed: bool,
) -> (Rect, Option<PanelRender<'_>>) {
    let Some(panel) = priors else {
        return (area, None);
    };

    let hint_text = if panel.is_empty() {
        panel::empty_hint()
    } else if collapsed {
        panel::collapsed_hint(panel)
    } else {
        match panel::fit(area.width, panel) {
            panel::Fit::Columns { priors, summary } => {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Min(1),
                        Constraint::Length(panel::panel_width(priors + usize::from(summary))),
                    ])
                    .split(area);
                return (
                    chunks[0],
                    Some(PanelRender::Columns {
                        area: chunks[1],
                        panel,
                        priors,
                        summary,
                    }),
                );
            }
            panel::Fit::Hint => panel::narrow_hint(panel),
        }
    };

    // One hint row at the bottom, when there is a row to spare.
    if area.height < 2 {
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
    (
        form,
        Some(PanelRender::Hint {
            area: hint,
            text: hint_text,
        }),
    )
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

    // Carve out the read-only priors panel (right, or a one-line hint).
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
        ]);

        // What the *active field* responds to, rather than a generic "Enter
        // interact" that is wrong on half of them. On a toggle Enter advances
        // and space is the only key that flips it; on a counter a bare number
        // adds and a leading `=` sets. Both shipped in habit capture v1 with no
        // hint at all, and a key you have to guess is a key nobody finds.
        match active_field_cfg.map(|f| &f.field_type) {
            Some(crate::config::FieldType::Toggle) => spans.extend([
                Span::styled("space", Style::default().fg(Color::Yellow)),
                Span::raw(" flip  "),
            ]),
            Some(crate::config::FieldType::Counter) => spans.extend([
                Span::styled("0-9", Style::default().fg(Color::Yellow)),
                Span::raw(" add  "),
                Span::styled("=", Style::default().fg(Color::Yellow)),
                Span::raw(" set  "),
            ]),
            _ => spans.extend([
                Span::styled("Enter", Style::default().fg(Color::Yellow)),
                Span::raw(" interact  "),
            ]),
        }
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

    // Fields after the footer: a textarea popout near the bottom of the form
    // runs over the footer and has to paint on top of it. The returned item
    // heights line the priors panel's rows up with the form.
    let item_heights =
        fields::render_fields(frame, chunks[1], &module.fields, form_state, has_picker);

    // Read-only priors panel: columns on the right, or a one-line hint.
    match panel_render {
        Some(PanelRender::Columns {
            area: panel_area,
            panel,
            priors,
            summary,
        }) => {
            let visible: Vec<&crate::config::FieldConfig> =
                visible_field_indices(&module.fields, &form_state.field_values)
                    .into_iter()
                    .filter_map(|ci| module.fields.get(ci))
                    .collect();
            let rows =
                panel::RowLayout::new(chunks[1], &item_heights, &visible, form_state.active_field);
            panel::render_panel(
                frame,
                panel_area,
                panel,
                (priors, summary),
                &rows,
                &form_state.field_values,
            );
        }
        Some(PanelRender::Hint {
            area: hint_area,
            text,
        }) => panel::render_hint(frame, hint_area, &text),
        None => {}
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
