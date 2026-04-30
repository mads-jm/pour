use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{App, FormState, PresetPickerState};
use crate::tui::form::FormAction;

// ── Render ────────────────────────────────────────────────────────────────────

pub(in crate::tui::form) fn render(
    frame: &mut Frame,
    area: Rect,
    picker: &PresetPickerState,
    axes: &[String],
) {
    use crate::data::preset_tree::TreeNode;

    if area.height < 6 || area.width < 30 {
        return;
    }

    let modal_width = (area.width * 2 / 3)
        .max(40)
        .min(area.width.saturating_sub(4));
    let modal_height = (area.height * 2 / 3)
        .max(8)
        .min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    let breadcrumb = super::build_breadcrumb(picker, axes);
    let title = format!(" Preset: {breadcrumb} ");
    frame.render_widget(Clear, modal_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title.as_str())
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, modal_area);

    let inner = Rect::new(
        modal_area.x + 1,
        modal_area.y + 1,
        modal_area.width.saturating_sub(2),
        modal_area.height.saturating_sub(2),
    );

    let list_height = inner.height.saturating_sub(1) as usize;

    let nodes = super::current_nodes(picker);
    let total = nodes.len();

    let items: Vec<ratatui::widgets::ListItem> = nodes
        .iter()
        .skip(picker.viewport_offset)
        .take(list_height)
        .enumerate()
        .map(|(i, node)| {
            let abs_i = i + picker.viewport_offset;
            let is_selected = abs_i == picker.selected;
            let style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };
            match node {
                TreeNode::Branch {
                    axis_value, count, ..
                } => ratatui::widgets::ListItem::new(Line::from(vec![
                    Span::styled(format!("  {axis_value}"), style),
                    Span::styled(
                        format!("  ({count})"),
                        if is_selected {
                            Style::default().fg(Color::DarkGray).bg(Color::Cyan)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                    Span::styled(" \u{25B8}", style),
                ])),
                TreeNode::Leaf {
                    preset_name,
                    description,
                } => {
                    let name_style = style;
                    if let Some(desc) = description {
                        let desc_style = if is_selected {
                            Style::default().fg(Color::DarkGray).bg(Color::Cyan)
                        } else {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::ITALIC)
                        };
                        ratatui::widgets::ListItem::new(ratatui::text::Text::from(vec![
                            Line::from(Span::styled(format!("  {preset_name}"), name_style)),
                            Line::from(Span::styled(format!("    {desc}"), desc_style)),
                        ]))
                    } else {
                        ratatui::widgets::ListItem::new(Line::from(Span::styled(
                            format!("  {preset_name}"),
                            name_style,
                        )))
                    }
                }
            }
        })
        .collect();

    let list_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(1),
    );
    frame.render_widget(ratatui::widgets::List::new(items), list_area);

    if total > list_height {
        let pct = (picker.viewport_offset * 100) / total.max(1);
        let scroll_text = format!(" {}/{} ({}%)", picker.selected + 1, total, pct);
        let indicator_area = Rect::new(
            modal_area.x
                + modal_area
                    .width
                    .saturating_sub(scroll_text.len() as u16 + 1),
            modal_area.y,
            scroll_text.len() as u16,
            1,
        );
        frame.render_widget(
            Paragraph::new(Span::styled(
                scroll_text,
                Style::default().fg(Color::DarkGray),
            )),
            indicator_area,
        );
    }

    let hint_area = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(1),
        inner.width,
        1,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("\u{2191}\u{2193}", Style::default().fg(Color::Yellow)),
            Span::raw(" nav  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" select  "),
            Span::styled("Bksp/\u{2190}", Style::default().fg(Color::Yellow)),
            Span::raw(" back  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ])),
        hint_area,
    );
}

// ── Key handler ───────────────────────────────────────────────────────────────

/// Handle key events while the preset picker drilldown is open.
///
/// All keys consumed. ↑↓ navigate, Enter drill/apply, Backspace/Left pop, Esc cancel.
pub(in crate::tui::form) fn handle_key(
    form_state: &mut FormState,
    module_key: &str,
    module: &crate::config::ModuleConfig,
    presets: &crate::data::presets::Presets,
    key: crossterm::event::KeyEvent,
) -> FormAction {
    use crate::data::preset_tree::TreeNode;
    use crossterm::event::KeyCode;

    let picker = match &mut form_state.preset_picker {
        Some(p) => p,
        None => return FormAction::None,
    };

    let nodes_len = super::current_nodes(picker).len();

    match key.code {
        KeyCode::Esc => {
            form_state.preset_picker = None;
            FormAction::None
        }
        KeyCode::Up => {
            if picker.selected > 0 {
                picker.selected -= 1;
                if picker.selected < picker.viewport_offset {
                    picker.viewport_offset = picker.selected;
                }
            }
            FormAction::None
        }
        KeyCode::Down => {
            if picker.selected + 1 < nodes_len {
                picker.selected += 1;
                if picker.selected >= picker.viewport_offset + 20 {
                    picker.viewport_offset = picker.selected.saturating_sub(19);
                }
            }
            FormAction::None
        }
        KeyCode::Enter => {
            let node = super::current_nodes(picker).get(picker.selected).cloned();
            match node {
                Some(TreeNode::Branch { .. }) => {
                    let selected = picker.selected;
                    let picker = form_state.preset_picker.as_mut().unwrap();
                    picker.path.push(selected);
                    picker.selected = 0;
                    picker.viewport_offset = 0;
                    FormAction::None
                }
                Some(TreeNode::Leaf { preset_name, .. }) => {
                    let preset_entry = presets
                        .get(module_key)
                        .into_iter()
                        .find(|p| p.name == preset_name);
                    form_state.selected_preset_name = Some(preset_name);
                    form_state.preset_picker = None;
                    App::apply_preset(form_state, &module.fields, preset_entry.as_ref());
                    form_state.active_field = 0;
                    form_state.active_config_idx = None;
                    FormAction::None
                }
                None => FormAction::None,
            }
        }
        KeyCode::Backspace | KeyCode::Left => {
            let picker = form_state.preset_picker.as_mut().unwrap();
            if picker.path.is_empty() {
                form_state.preset_picker = None;
            } else {
                let prev_selected = picker.path.pop().unwrap_or(0);
                picker.selected = prev_selected;
                picker.viewport_offset = prev_selected.saturating_sub(10);
            }
            FormAction::None
        }
        _ => FormAction::None,
    }
}
