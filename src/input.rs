use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::Backend;
use crate::app::{App, AppMode, SortMode};
use crate::ui::events::AppEvent;
use crate::ui::components::settings::{SettingAction, get_setting_action};

pub async fn handle_event<B: Backend>(app: &mut App<B>, event: Event) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    match event {
        Event::Key(key) if key.kind == event::KeyEventKind::Press => handle_key(app, key).await,
        Event::Mouse(mouse) => handle_mouse(app, mouse).await,
        _ => {}
    }
}

pub async fn handle_mouse<B: Backend>(app: &mut App<B>, mouse: event::MouseEvent) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    use event::MouseEventKind;
    match mouse.kind {
        MouseEventKind::ScrollDown
            if app.mode == AppMode::Normal && app.pr_list.selected_index() < app.pr_list.items().len().saturating_sub(1) => {
                app.pr_list.select_next();
                app.trigger_details_fetch().await;
            }
        MouseEventKind::ScrollUp
            if app.mode == AppMode::Normal && app.pr_list.selected_index() > 0 => {
                app.pr_list.select_prev();
                app.trigger_details_fetch().await;
            }
        _ => {}
    }
}

pub async fn handle_key<B: Backend>(app: &mut App<B>, key: KeyEvent) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    let old_index = app.pr_list.selected_index();

    match app.mode {
        AppMode::Search => handle_search_key(app, key).await,
        AppMode::Follow => handle_follow_key(app, key).await,
        AppMode::Help => handle_help_key(app, key),
        AppMode::Archive => handle_archive_key(app, key),
        AppMode::Settings => handle_settings_key(app, key),
        AppMode::Diagnostic => handle_diagnostic_key(app, key),
        AppMode::LogDetail => handle_log_detail_key(app, key),
        AppMode::Normal => handle_normal_key(app, key).await,
        AppMode::AddQueryName => handle_add_query_name_key(app, key).await,
        AppMode::AddQuerySearch => handle_add_query_search_key(app, key).await,
        AppMode::ConfirmQuery => handle_confirm_query_key(app, key).await,
    }

    if old_index != app.pr_list.selected_index() {
        app.trigger_details_fetch().await;
    }
}

async fn handle_search_key<B: Backend>(app: &mut App<B>, key: KeyEvent) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.input_buffer.clear();
        }
        KeyCode::Enter => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(c) => {
            app.input_buffer.push(c);
        }
        _ => {}
    }
}

async fn handle_follow_key<B: Backend>(app: &mut App<B>, key: KeyEvent) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.input_buffer.clear();
        }
        KeyCode::Enter => {
            let input = app.input_buffer.clone();
            app.input_buffer.clear();
            app.mode = AppMode::Normal;
            app.follow_pr(&input).await;
        }
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(c) => {
            app.input_buffer.push(c);
        }
        _ => {}
    }
}

fn handle_help_key<B: Backend>(app: &mut App<B>, key: KeyEvent) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    if matches!(key.code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')) {
        app.mode = AppMode::Normal;
    }
}

fn handle_archive_key<B: Backend>(app: &mut App<B>, key: KeyEvent) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    match key.code {
        KeyCode::Char('j') | KeyCode::Down if app.archive_list.selected_index() < app.archive_list.items().len().saturating_sub(1) => {
            app.archive_list.select_next();
        }
        KeyCode::Char('k') | KeyCode::Up if app.archive_list.selected_index() > 0 => {
            app.archive_list.select_prev();
        }
        KeyCode::Char('g') => app.archive_list.set_selected_index(0),
        KeyCode::Char('G') => app.archive_list.set_selected_index(app.archive_list.items().len().saturating_sub(1)),
        KeyCode::Char('d') if app.is_writer && app.archive_list.selected_index() < app.archive_list.items().len() 
            && app.archive_list.remove_selected().is_some() => {
                let _ = app.state_repo.save_archive(app.archive_list.items());
        }
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Char('q') => app.should_quit = true,
        _ => {}
    }
}

fn handle_settings_key<B: Backend>(app: &mut App<B>, key: KeyEvent) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            let max = 11 + app.config.queries.len();
            if app.settings_selected_index < max.saturating_sub(1) {
                app.settings_selected_index += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up
            if app.settings_selected_index > 0 => {
                app.settings_selected_index -= 1;
            }
        KeyCode::Enter | KeyCode::Char(' ') => {
            match get_setting_action(&app.config, app.settings_selected_index) {
                SettingAction::None => {}
                SettingAction::ToggleNerdFonts => app.config.use_nerd_fonts = !app.config.use_nerd_fonts,
                SettingAction::ToggleStatusBar => app.config.show_status_bar = !app.config.show_status_bar,
                SettingAction::CycleTheme => {
                    app.config.theme = match app.config.theme.as_str() {
                        "dark" => "nord".to_string(),
                        "nord" => "dracula".to_string(),
                        _ => "dark".to_string(),
                    };
                }
                SettingAction::ToggleColumn(col) => {
                    if let Some(pos) = app.config.visible_columns.iter().position(|c| c == &col) {
                        app.config.visible_columns.remove(pos);
                    } else {
                        app.config.visible_columns.push(col);
                    }
                }
                SettingAction::ToggleQuery(q_idx) => {
                    if q_idx < app.config.queries.len() {
                        app.config.queries[q_idx].enabled = !app.config.queries[q_idx].enabled;
                    }
                }
                SettingAction::AddQuery => {
                    app.query_name_buffer.clear();
                    app.query_search_buffer.clear();
                    app.query_test_results = None;
                    app.query_test_error = None;
                    app.is_testing_query = false;
                    app.mode = AppMode::AddQueryName;
                }
            }
        }
        KeyCode::Char('D') => {
            app.diagnostic_selected_index = 0;
            app.mode = AppMode::Diagnostic;
        }
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            // Save config on exit
            let config_dir = crate::storage::get_config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            let config_path = config_dir.join("config.toml");
            let _ = std::fs::write(config_path, toml::to_string(&app.config).unwrap_or_default());
        }
        _ => {}
    }
}

async fn handle_add_query_name_key<B: Backend>(app: &mut App<B>, key: KeyEvent) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    match key.code {
        KeyCode::Enter => {
            if !app.query_name_buffer.is_empty() {
                app.mode = AppMode::AddQuerySearch;
            }
        }
        KeyCode::Esc => {
            app.mode = AppMode::Settings;
        }
        KeyCode::Backspace => {
            app.query_name_buffer.pop();
        }
        KeyCode::Char(c) => {
            app.query_name_buffer.push(c);
        }
        _ => {}
    }
}

async fn handle_add_query_search_key<B: Backend>(app: &mut App<B>, key: KeyEvent) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    match key.code {
        KeyCode::Enter => {
            if !app.query_search_buffer.is_empty() {
                app.is_testing_query = true;
                app.query_test_results = None;
                app.query_test_error = None;
                app.mode = AppMode::ConfirmQuery;
                
                let github = app.github.clone();
                let tx = app.event_tx.clone();
                let query = app.query_search_buffer.clone();
                
                tokio::spawn(async move {
                    match github.fetch_prs_by_query(&query, Some(5)).await {
                        Ok(prs) => {
                            let _ = tx.send(AppEvent::QueryTested(Ok(prs))).await;
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::QueryTested(Err(e.to_string()))).await;
                        }
                    }
                });
            }
        }
        KeyCode::Esc => {
            app.mode = AppMode::AddQueryName;
        }
        KeyCode::Backspace => {
            app.query_search_buffer.pop();
        }
        KeyCode::Char(c) => {
            app.query_search_buffer.push(c);
        }
        _ => {}
    }
}

async fn handle_confirm_query_key<B: Backend>(app: &mut App<B>, key: KeyEvent) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter if !app.is_testing_query && app.query_test_error.is_none() => {
            // Add the query
            app.config.queries.push(crate::config::QueryConfig {
                name: app.query_name_buffer.clone(),
                search: app.query_search_buffer.clone(),
                interval: "60s".to_string(),
                enabled: true,
            });
            app.mode = AppMode::Settings;
            // Trigger a refresh for the new query
            if app.is_writer {
                app.handle_config_reload(app.config.clone());
            }
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            app.mode = AppMode::AddQuerySearch;
        }
        _ => {}
    }
}

fn handle_diagnostic_key<B: Backend>(app: &mut App<B>, key: KeyEvent) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            let max = crate::logging::get_gh_calls().len();
            if app.diagnostic_selected_index < max.saturating_sub(1) {
                app.diagnostic_selected_index += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up
            if app.diagnostic_selected_index > 0 => {
                app.diagnostic_selected_index -= 1;
        }
        KeyCode::Enter => {
            app.mode = AppMode::LogDetail;
        }
        KeyCode::Char('y') => {
            let calls = crate::logging::get_gh_calls();
            let calls_reversed: Vec<_> = calls.iter().rev().collect();
            if let Some(call) = calls_reversed.get(app.diagnostic_selected_index) {
                let cmd = call.command.clone();
                app.copy_to_clipboard(&cmd);
            }
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.mode = AppMode::Settings;
        }
        _ => {}
    }
}

fn handle_log_detail_key<B: Backend>(app: &mut App<B>, key: KeyEvent) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
        app.mode = AppMode::Diagnostic;
    }
}

async fn handle_normal_key<B: Backend>(app: &mut App<B>, key: KeyEvent) 
where B::Error: std::error::Error + Send + Sync + 'static 
{
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('?') => {
            app.mode = AppMode::Help;
        }
        KeyCode::Char('/') => {
            app.mode = AppMode::Search;
        }
        KeyCode::Char('f') if app.is_writer => {
            app.mode = AppMode::Follow;
        }
        KeyCode::Char('S') => {
            app.mode = AppMode::Settings;
        }
        KeyCode::Char('A') => {
            if let Ok(archived) = app.state_repo.load_archive() {
                app.archive_list.set_prs(archived);
            }
            app.archive_list.set_selected_index(0);
            app.mode = AppMode::Archive;
        }
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Char('s') => {
            app.sort_mode = match app.sort_mode {
                SortMode::Updated => SortMode::Created,
                SortMode::Created => SortMode::Priority,
                SortMode::Priority => SortMode::Repo,
                SortMode::Repo => SortMode::Updated,
            };
            app.sort_prs();
        }
        KeyCode::Tab => {
            app.detail_focused = !app.detail_focused;
            app.detail_scroll = 0;
        }
        KeyCode::Char('o') => {
            if let Some(pr) = app.pr_list.selected_pr() {
                let url = pr.url.clone();
                let github = app.github.clone();
                tokio::spawn(async move {
                    let _ = github.open_pr_in_browser(&url).await;
                });
            }
        }
        KeyCode::Char('y') => {
            if let Some(pr) = app.pr_list.selected_pr() {
                let url = pr.url.clone();
                app.copy_to_clipboard(&url);
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if app.detail_focused {
                app.detail_scroll = app.detail_scroll.saturating_add(1);
            } else if app.pr_list.selected_index() < app.pr_list.items().len().saturating_sub(1) {
                app.pr_list.select_next();
                app.detail_scroll = 0;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.detail_focused {
                app.detail_scroll = app.detail_scroll.saturating_sub(1);
            } else if app.pr_list.selected_index() > 0 {
                app.pr_list.select_prev();
                app.detail_scroll = 0;
            }
        }
        KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.config.group_by = match app.config.group_by {
                crate::config::GroupMode::None => crate::config::GroupMode::Repo,
                crate::config::GroupMode::Repo => crate::config::GroupMode::Author,
                crate::config::GroupMode::Author => crate::config::GroupMode::Status,
                crate::config::GroupMode::Status => crate::config::GroupMode::MyVsOther,
                crate::config::GroupMode::MyVsOther => crate::config::GroupMode::None,
            };
            app.sort_prs();
        }
        KeyCode::Char('g') => app.pr_list.set_selected_index(0),
        KeyCode::Char('G') => app.pr_list.set_selected_index(app.pr_list.items().len().saturating_sub(1)),
        KeyCode::Char('m') if app.is_writer => {
            let mut prs = app.pr_list.items().to_vec();
            if let Some(pr) = prs.get_mut(app.pr_list.selected_index()) {
                pr.last_seen_at = Some(pr.updated_at.clone());
                pr.last_seen_unresolved_count = pr.unresolved_count;
                pr.last_seen_total_resolvable_count = pr.total_resolvable_count;
                pr.last_seen_conversational_count = pr.conversational_count;
                app.pr_list.set_prs(prs);
                let _ = app.state_repo.save_state(app.pr_list.items());
            }
        }
        KeyCode::Char('M') if app.is_writer => {
            let mut prs = app.pr_list.items().to_vec();
            for pr in &mut prs {
                pr.last_seen_at = Some(pr.updated_at.clone());
                pr.last_seen_unresolved_count = pr.unresolved_count;
                pr.last_seen_total_resolvable_count = pr.total_resolvable_count;
                pr.last_seen_conversational_count = pr.conversational_count;
            }
            app.pr_list.set_prs(prs);
            let _ = app.state_repo.save_state(app.pr_list.items());
        }
        KeyCode::Char('u') if app.is_writer && app.pr_list.selected_index() < app.pr_list.items().len() => {
            if let Some(pr) = app.pr_list.remove_selected() {
                app.archive_list.insert_at_front(pr.clone());
                let _ = app.state_repo.archive_pr(pr);
                let _ = app.state_repo.save_state(app.pr_list.items());
            }
        }
        _ => {}
    }
}
