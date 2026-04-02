pub mod configure;
pub mod dashboard;
pub mod form;
pub mod summary;

use crate::app::{App, Screen};
use ratatui::Frame;

/// Top-level action returned by event handling.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// No state change needed.
    None,
    /// The user wants to quit the application.
    Quit,
    /// Submit the current form for writing.
    Submit,
    /// Navigate to a different screen.
    Navigate(Screen),
    /// Save the current configure state to disk.
    Save,
    /// Fetch a directory listing for the given vault-relative path.
    Browse(String),
    /// Add a new default field to the current module.
    AddField,
    /// Remove a field at the given index from the current module.
    RemoveField(usize),
    /// Swap two fields at the given indices in the current module.
    ReorderFields(usize, usize),
    /// Delete the current module entirely.
    DeleteModule,
    /// Reorder modules: swap selected module in the given direction, then persist.
    ReorderModules(dashboard::MoveDirection),
    /// Open the new module creation screen.
    NewModule,
    /// Save the new module being configured to disk.
    SaveNewModule,
}

/// Dispatch rendering to the correct view based on the current screen.
pub fn render(app: &App, frame: &mut Frame) {
    match app.screen {
        Screen::Dashboard => dashboard::render(app, frame),
        Screen::Form => form::render(app, frame),
        Screen::Summary => summary::render(app, frame),
        Screen::Configure => configure::render(app, frame),
    }
}

/// Dispatch a key event to the correct handler based on the current screen.
///
/// Returns an `Action` that the main loop should act on.
pub fn handle_event(app: &mut App, key: crossterm::event::KeyEvent) -> Action {
    match app.screen {
        Screen::Dashboard => match dashboard::handle_key(app, key) {
            dashboard::DashboardAction::Quit => Action::Quit,
            dashboard::DashboardAction::SelectModule => {
                let module_key = app.module_keys.get(app.selected_module).cloned();
                if let Some(key) = module_key {
                    app.form_state = app.init_form(&key);
                    app.screen = Screen::Form;
                }
                Action::Navigate(Screen::Form)
            }
            dashboard::DashboardAction::ConfigureModule => {
                let module_key = app.module_keys.get(app.selected_module).cloned();
                if let Some(key) = module_key {
                    app.configure_state = app.init_configure(&key);
                    app.screen = Screen::Configure;
                    Action::Navigate(Screen::Configure)
                } else {
                    Action::None
                }
            }
            dashboard::DashboardAction::ConfigureVault => {
                app.configure_state = Some(app.init_vault_configure());
                app.screen = Screen::Configure;
                Action::Navigate(Screen::Configure)
            }
            dashboard::DashboardAction::ReorderModule(dir) => {
                Action::ReorderModules(dir)
            }
            dashboard::DashboardAction::NewModule => Action::NewModule,
            dashboard::DashboardAction::None => Action::None,
        },

        Screen::Form => match form::handle_key(app, key) {
            form::FormAction::Cancel => {
                app.form_state = None;
                app.screen = Screen::Dashboard;
                Action::Navigate(Screen::Dashboard)
            }
            form::FormAction::Submit => Action::Submit,
            form::FormAction::None => Action::None,
        },

        Screen::Summary => match summary::handle_key(key) {
            summary::SummaryAction::Quit => Action::Quit,
            summary::SummaryAction::Dashboard => {
                app.summary_state = None;
                app.screen = Screen::Dashboard;
                Action::Navigate(Screen::Dashboard)
            }
            summary::SummaryAction::AnotherEntry => {
                let module_key = app.module_keys.get(app.selected_module).cloned();
                if let Some(key) = module_key {
                    app.form_state = app.init_form(&key);
                    app.screen = Screen::Form;
                }
                app.summary_state = None;
                Action::Navigate(Screen::Form)
            }
            summary::SummaryAction::None => Action::None,
        },

        Screen::Configure => match configure::handle_key(app, key) {
            configure::ConfigureAction::Cancel => {
                app.configure_state = None;
                app.screen = Screen::Dashboard;
                Action::Navigate(Screen::Dashboard)
            }
            configure::ConfigureAction::Save => Action::Save,
            configure::ConfigureAction::BrowseDirectory(path) => Action::Browse(path),
            configure::ConfigureAction::AddField => Action::AddField,
            configure::ConfigureAction::RemoveField(idx) => Action::RemoveField(idx),
            configure::ConfigureAction::ReorderFields(a, b) => Action::ReorderFields(a, b),
            configure::ConfigureAction::DeleteModule => Action::DeleteModule,
            configure::ConfigureAction::SaveNewModule => Action::SaveNewModule,
            configure::ConfigureAction::None => Action::None,
        },
    }
}
