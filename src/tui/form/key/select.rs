use crate::app::{App, FormState, SubFormState};
use crate::tui::form::FormAction;

/// Inline cycle for a select field (Left/Right when dropdown closed).
pub(super) fn inline_cycle(form_state: &mut FormState, field_name: &str, delta: i32) {
    if let Some(opts) = form_state.field_options.get(field_name).cloned() {
        if opts.is_empty() {
            return;
        }
        let current = form_state
            .field_values
            .get(field_name)
            .cloned()
            .unwrap_or_default();
        let idx = opts.iter().position(|o| o == &current).unwrap_or(0);
        let new_idx = if delta < 0 {
            if idx == 0 { opts.len() - 1 } else { idx - 1 }
        } else {
            (idx + 1) % opts.len()
        };
        form_state
            .field_values
            .insert(field_name.to_string(), opts[new_idx].clone());
    }
}

/// Handle Enter for a select field.
///
/// Takes `&mut App` to avoid split-borrow issues (template lookup needs
/// `app.config`, create-template opens `app.form_state.sub_form`).
pub(super) fn handle_select_enter(
    app: &mut App,
    module_key: &str,
    field_name: &str,
    is_allow_create: bool,
    is_static_allow_create: bool,
    active_config_idx: Option<usize>,
) -> FormAction {
    // Read search buffer
    let search = app
        .form_state
        .as_ref()
        .and_then(|fs| fs.search_buffers.get(field_name).cloned())
        .unwrap_or_default();

    if !is_allow_create || search.is_empty() {
        let form_state = app.form_state.as_mut().unwrap();
        form_state.dropdown_open = !form_state.dropdown_open;
        return FormAction::None;
    }

    // Collect filtered options
    let filtered: Vec<String> = app
        .form_state
        .as_ref()
        .and_then(|fs| fs.field_options.get(field_name))
        .map(|opts| {
            opts.iter()
                .filter(|o| o.to_lowercase().contains(&search.to_lowercase()))
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    if filtered.is_empty() {
        return handle_novel_value(
            app,
            module_key,
            field_name,
            &search,
            is_static_allow_create,
            active_config_idx,
        );
    }

    // Matches exist — confirm the highlighted one
    let current = app
        .form_state
        .as_ref()
        .and_then(|fs| fs.field_values.get(field_name).cloned())
        .unwrap_or_default();
    let best = if filtered.contains(&current) {
        current
    } else {
        filtered.into_iter().next().unwrap_or_default()
    };
    let form_state = app.form_state.as_mut().unwrap();
    form_state.field_values.insert(field_name.to_string(), best);
    form_state.search_buffers.remove(field_name);
    form_state.dropdown_open = false;
    FormAction::None
}

fn handle_novel_value(
    app: &mut App,
    module_key: &str,
    field_name: &str,
    search: &str,
    is_static_allow_create: bool,
    active_config_idx: Option<usize>,
) -> FormAction {
    // Check for create_template on the field
    let create_template = app
        .config
        .modules
        .get(module_key)
        .and_then(|m| m.fields.iter().find(|f| f.name == field_name))
        .and_then(|f| f.create_template.clone());

    if let Some(ref tpl_name) = create_template {
        let term_size = crossterm::terminal::size().unwrap_or((80, 24));
        if term_size.1 >= 10 && term_size.0 >= 30 {
            let template = app
                .config
                .templates
                .as_ref()
                .and_then(|t| t.get(tpl_name.as_str()))
                .cloned();

            if let Some(template) = template {
                let form_state = app.form_state.as_mut().unwrap();
                form_state.dropdown_open = false;
                form_state.sub_form = Some(SubFormState::new(
                    tpl_name.clone(),
                    search.to_string(),
                    field_name.to_string(),
                    &template,
                ));
                form_state.search_buffers.remove(field_name);
                return FormAction::None;
            }
        }
    }

    let form_state = app.form_state.as_mut().unwrap();

    if is_static_allow_create {
        if let Some(opts) = form_state.field_options.get_mut(field_name)
            && !opts.iter().any(|o| o == search)
        {
            opts.push(search.to_string());
        }
        form_state
            .field_values
            .insert(field_name.to_string(), search.to_string());
        form_state.search_buffers.remove(field_name);
        form_state.dropdown_open = false;
        if let Some(idx) = active_config_idx {
            return FormAction::AppendStaticOption {
                field_index: idx,
                value: search.to_string(),
            };
        }
        return FormAction::None;
    }

    // Fallback: accept typed text as novel value
    form_state
        .field_values
        .insert(field_name.to_string(), search.to_string());
    form_state.search_buffers.remove(field_name);
    form_state.dropdown_open = false;
    FormAction::None
}
