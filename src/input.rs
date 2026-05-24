use crate::app::{App, AppMode, SortMode};
use crate::ui::components::settings::{QUERIES_START, SettingAction, get_setting_action};
use crate::ui::events::AppEvent;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::Backend;

const fn switch_to_prs<B: Backend>(app: &mut App<B>)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    app.mode = AppMode::Normal;
}

fn switch_to_archive<B: Backend>(app: &mut App<B>)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    if let Ok(archived) = app.state_repo.load_archive() {
        app.archive_list.set_prs(archived);
    }
    app.archive_list.set_selected_index(0);
    app.mode = AppMode::Archive;
}

const fn switch_to_settings<B: Backend>(app: &mut App<B>)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    app.mode = AppMode::Settings;
}

pub async fn handle_event<B: Backend>(app: &mut App<B>, event: Event)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    match event {
        Event::Key(key) if key.kind == event::KeyEventKind::Press => handle_key(app, key).await,
        Event::Mouse(mouse) => handle_mouse(app, mouse).await,
        _ => {}
    }
}

pub async fn handle_mouse<B: Backend>(app: &mut App<B>, mouse: event::MouseEvent)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    use event::MouseEventKind;
    match mouse.kind {
        MouseEventKind::ScrollDown
            if app.mode == AppMode::Normal
                && app.pr_list.selected_index() < app.pr_list.items().len().saturating_sub(1) =>
        {
            app.pr_list.select_next();
            app.trigger_details_fetch().await;
        }
        MouseEventKind::ScrollUp
            if app.mode == AppMode::Normal && app.pr_list.selected_index() > 0 =>
        {
            app.pr_list.select_prev();
            app.trigger_details_fetch().await;
        }
        _ => {}
    }
}

pub async fn handle_key<B: Backend>(app: &mut App<B>, key: KeyEvent)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    let old_index = app.pr_list.selected_index();

    match app.mode {
        AppMode::Search => handle_search_key(app, key).await,
        AppMode::Follow => handle_follow_key(app, key).await,
        AppMode::Help => handle_help_key(app, key),
        AppMode::Archive => handle_archive_key(app, key),
        AppMode::Settings => handle_settings_key(app, key),
        AppMode::ThemePicker => handle_theme_picker_key(app, key),
        AppMode::Diagnostic => handle_diagnostic_key(app, key),
        AppMode::LogDetail => handle_log_detail_key(app, key),
        AppMode::Normal => handle_normal_key(app, key).await,
        AppMode::AddQueryName => handle_add_query_name_key(app, key).await,
        AppMode::AddQuerySearch => handle_add_query_search_key(app, key).await,
        AppMode::ConfirmQuery => handle_confirm_query_key(app, key).await,
        AppMode::DeleteQueryConfirm => handle_delete_query_confirm_key(app, key),
        AppMode::EditMaxAgeDays => handle_edit_max_age_days_key(app, key),
    }

    if old_index != app.pr_list.selected_index() {
        app.trigger_details_fetch().await;
    }
}

async fn handle_search_key<B: Backend>(app: &mut App<B>, key: KeyEvent)
where
    B::Error: std::error::Error + Send + Sync + 'static,
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
where
    B::Error: std::error::Error + Send + Sync + 'static,
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

const fn handle_help_key<B: Backend>(app: &mut App<B>, key: KeyEvent)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    if matches!(key.code, KeyCode::Esc | KeyCode::Char('?' | 'q')) {
        app.mode = AppMode::Normal;
    }
}

fn handle_archive_key<B: Backend>(app: &mut App<B>, key: KeyEvent)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    match key.code {
        KeyCode::Char('j') | KeyCode::Down
            if app.archive_list.selected_index()
                < app.archive_list.items().len().saturating_sub(1) =>
        {
            app.archive_list.select_next();
        }
        KeyCode::Char('k') | KeyCode::Up if app.archive_list.selected_index() > 0 => {
            app.archive_list.select_prev();
        }
        KeyCode::Char('g') => app.archive_list.set_selected_index(0),
        KeyCode::Char('G') => {
            app.archive_list.set_selected_index(app.archive_list.items().len().saturating_sub(1));
        }
        KeyCode::Char('d')
            if app.archive_list.selected_index() < app.archive_list.items().len()
                && app.archive_list.remove_selected().is_some() =>
        {
            let _ = app.state_repo.save_archive(app.archive_list.items());
        }
        KeyCode::Char('f')
            if app.archive_list.selected_index() < app.archive_list.items().len() =>
        {
            if let Some(mut pr) = app.archive_list.remove_selected() {
                if !pr.matched_queries.iter().any(|q| q == "manual") {
                    pr.matched_queries.push("manual".to_string());
                }
                app.pr_list.insert_at_front(pr);
                let _ = app.state_repo.save_archive(app.archive_list.items());
                let _ = app.state_repo.save_state(app.pr_list.items());
            }
        }
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => switch_to_prs(app),
        KeyCode::Right | KeyCode::Char('l') => switch_to_settings(app),
        KeyCode::Char('q') => app.should_quit = true,
        _ => {}
    }
}

fn handle_settings_key<B: Backend>(app: &mut App<B>, key: KeyEvent)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            let max = QUERIES_START + 1 + app.config.queries.len();
            if app.settings_selected_index < max.saturating_sub(1) {
                app.settings_selected_index += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up if app.settings_selected_index > 0 => {
            app.settings_selected_index -= 1;
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            match get_setting_action(&app.config, app.settings_selected_index) {
                SettingAction::None => {}
                SettingAction::ToggleNerdFonts => {
                    app.config.use_nerd_fonts = !app.config.use_nerd_fonts;
                }
                SettingAction::ToggleOpenInBrowserMarksSeen => {
                    app.config.attention.open_in_browser_marks_seen =
                        !app.config.attention.open_in_browser_marks_seen;
                }
                SettingAction::ToggleStatusBar => {
                    app.config.show_status_bar = !app.config.show_status_bar;
                }
                SettingAction::OpenThemePicker => {
                    let themes = ratatui_themes::ThemeName::all();
                    app.theme_picker_original = Some(app.config.theme.clone());
                    app.theme_picker_index =
                        themes.iter().position(|t| t.slug() == app.config.theme).unwrap_or(0);
                    app.mode = AppMode::ThemePicker;
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
                SettingAction::EditMaxAgeDays => {
                    app.max_age_days_buffer = match app.config.max_age_days {
                        Some(n) => n.to_string(),
                        None => String::new(),
                    };
                    app.mode = AppMode::EditMaxAgeDays;
                }
            }
        }
        KeyCode::Char('e') => {
            if let SettingAction::ToggleQuery(q_idx) =
                get_setting_action(&app.config, app.settings_selected_index)
            {
                app.editing_query_index = Some(q_idx);
                app.query_name_buffer = app.config.queries[q_idx].name.clone();
                app.query_search_buffer = app.config.queries[q_idx].search.clone();
                app.query_test_results = None;
                app.query_test_error = None;
                app.is_testing_query = false;
                app.mode = AppMode::AddQueryName;
            }
        }
        KeyCode::Char('d') => {
            if let SettingAction::ToggleQuery(q_idx) =
                get_setting_action(&app.config, app.settings_selected_index)
            {
                app.deleting_query_index = Some(q_idx);
                app.mode = AppMode::DeleteQueryConfirm;
            }
        }
        KeyCode::Char('D') => {
            app.diagnostic_selected_index = 0;
            app.mode = AppMode::Diagnostic;
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.save_config();
            switch_to_archive(app);
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Esc => {
            app.save_config();
            switch_to_prs(app);
        }
        _ => {}
    }
}

async fn handle_add_query_name_key<B: Backend>(app: &mut App<B>, key: KeyEvent)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    match key.code {
        KeyCode::Enter if !app.query_name_buffer.is_empty() => {
            app.mode = AppMode::AddQuerySearch;
        }
        KeyCode::Esc => {
            app.editing_query_index = None;
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
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    match key.code {
        KeyCode::Enter if !app.query_search_buffer.is_empty() => {
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
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter
            if !app.is_testing_query && app.query_test_error.is_none() =>
        {
            if let Some(idx) = app.editing_query_index.take() {
                if idx < app.config.queries.len() {
                    app.config.queries[idx].name.clone_from(&app.query_name_buffer);
                    app.config.queries[idx].search.clone_from(&app.query_search_buffer);
                }
            } else {
                app.config.queries.push(crate::config::QueryConfig {
                    name: app.query_name_buffer.clone(),
                    search: app.query_search_buffer.clone(),
                    interval: "60s".to_string(),
                    enabled: true,
                });
            }
            app.mode = AppMode::Settings;
            app.save_config();
            app.handle_config_reload(app.config.clone());
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            app.mode = AppMode::AddQuerySearch;
        }
        _ => {}
    }
}

fn handle_delete_query_confirm_key<B: Backend>(app: &mut App<B>, key: KeyEvent)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => {
            if let Some(idx) = app.deleting_query_index.take()
                && idx < app.config.queries.len()
            {
                app.config.queries.remove(idx);
                let max_idx = (11 + app.config.queries.len()).saturating_sub(1);
                if app.settings_selected_index > max_idx {
                    app.settings_selected_index = max_idx;
                }
                app.save_config();
                app.handle_config_reload(app.config.clone());
            }
            app.mode = AppMode::Settings;
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            app.deleting_query_index = None;
            app.mode = AppMode::Settings;
        }
        _ => {}
    }
}

fn handle_edit_max_age_days_key<B: Backend>(app: &mut App<B>, key: KeyEvent)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    match key.code {
        KeyCode::Enter => {
            if app.max_age_days_buffer.is_empty() {
                app.config.max_age_days = None;
            } else if let Ok(n) = app.max_age_days_buffer.parse::<u32>() {
                app.config.max_age_days = if n == 0 { None } else { Some(n) };
            }
            app.save_config();
            app.handle_config_reload(app.config.clone());
            app.mode = AppMode::Settings;
        }
        KeyCode::Esc => {
            app.mode = AppMode::Settings;
        }
        KeyCode::Backspace => {
            app.max_age_days_buffer.pop();
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            app.max_age_days_buffer.push(c);
        }
        _ => {}
    }
}

fn handle_theme_picker_key<B: Backend>(app: &mut App<B>, key: KeyEvent)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    let themes = ratatui_themes::ThemeName::all();
    match key.code {
        KeyCode::Char('j') | KeyCode::Down
            if app.theme_picker_index < themes.len().saturating_sub(1) =>
        {
            app.theme_picker_index += 1;
            app.config.theme = themes[app.theme_picker_index].slug().to_string();
        }
        KeyCode::Char('k') | KeyCode::Up if app.theme_picker_index > 0 => {
            app.theme_picker_index -= 1;
            app.config.theme = themes[app.theme_picker_index].slug().to_string();
        }
        KeyCode::Enter => {
            app.theme_picker_original = None;
            app.mode = AppMode::Settings;
            app.save_config();
        }
        KeyCode::Esc => {
            if let Some(original) = app.theme_picker_original.take() {
                app.config.theme = original;
            }
            app.mode = AppMode::Settings;
        }
        _ => {}
    }
}

fn handle_diagnostic_key<B: Backend>(app: &mut App<B>, key: KeyEvent)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            let max = crate::logging::get_gh_calls().len();
            if app.diagnostic_selected_index < max.saturating_sub(1) {
                app.diagnostic_selected_index += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up if app.diagnostic_selected_index > 0 => {
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

const fn handle_log_detail_key<B: Backend>(app: &mut App<B>, key: KeyEvent)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
        app.mode = AppMode::Diagnostic;
    }
}

async fn handle_normal_key<B: Backend>(app: &mut App<B>, key: KeyEvent)
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('?') => {
            app.mode = AppMode::Help;
        }
        KeyCode::Char('/') => {
            app.mode = AppMode::Search;
        }
        KeyCode::Char('f') => {
            app.mode = AppMode::Follow;
        }
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.diagnostic_selected_index = 0;
            app.mode = AppMode::Diagnostic;
        }
        KeyCode::Left | KeyCode::Char('h') => switch_to_settings(app),
        KeyCode::Right | KeyCode::Char('l') => switch_to_archive(app),
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
                if app.config.attention.open_in_browser_marks_seen {
                    let now = chrono::Utc::now();
                    let mut prs = app.pr_list.items().to_vec();
                    if let Some(pr) = prs.get_mut(app.pr_list.selected_index()) {
                        pr.last_seen_at = Some(pr.updated_at.clone());
                        pr.last_seen_unresolved_count = pr.unresolved_count;
                        pr.last_seen_total_resolvable_count = pr.total_resolvable_count;
                        pr.last_seen_conversational_count = pr.conversational_count;
                        crate::domain::attention::apply_mark_seen(&mut pr.attention_state, now);
                        app.pr_list.set_prs(prs);
                        let _ = app.state_repo.save_state(app.pr_list.items());
                    }
                }
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
        KeyCode::Char('G') => {
            app.pr_list.set_selected_index(app.pr_list.items().len().saturating_sub(1));
        }
        KeyCode::Char('m') => {
            let now = chrono::Utc::now();
            let mut prs = app.pr_list.items().to_vec();
            if let Some(pr) = prs.get_mut(app.pr_list.selected_index()) {
                pr.last_seen_at = Some(pr.updated_at.clone());
                pr.last_seen_unresolved_count = pr.unresolved_count;
                pr.last_seen_total_resolvable_count = pr.total_resolvable_count;
                pr.last_seen_conversational_count = pr.conversational_count;
                crate::domain::attention::apply_mark_seen(&mut pr.attention_state, now);
                app.pr_list.set_prs(prs);
                let _ = app.state_repo.save_state(app.pr_list.items());
            }
        }
        KeyCode::Char('M') => {
            let now = chrono::Utc::now();
            let mut prs = app.pr_list.items().to_vec();
            for pr in &mut prs {
                pr.last_seen_at = Some(pr.updated_at.clone());
                pr.last_seen_unresolved_count = pr.unresolved_count;
                pr.last_seen_total_resolvable_count = pr.total_resolvable_count;
                pr.last_seen_conversational_count = pr.conversational_count;
                crate::domain::attention::apply_mark_seen(&mut pr.attention_state, now);
            }
            app.pr_list.set_prs(prs);
            let _ = app.state_repo.save_state(app.pr_list.items());
        }
        KeyCode::Char('u') if app.pr_list.selected_index() < app.pr_list.items().len() => {
            if let Some(mut pr) = app.pr_list.remove_selected() {
                crate::domain::attention::apply_archive(&mut pr.attention_state);
                app.archive_list.insert_at_front(pr.clone());
                let _ = app.state_repo.archive_pr(pr);
                let _ = app.state_repo.save_state(app.pr_list.items());
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::domain::attention::AttentionState;
    use crate::domain::ports::{MockGithubProvider, MockStateRepository};
    use crate::domain::pr::{CIStatus, MergeableStatus, PRStatus, PullRequest, ReviewStatus};
    use ratatui::backend::TestBackend;
    use std::sync::Arc;

    fn create_test_pr() -> PullRequest {
        PullRequest {
            id: "1".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            author: "alice".to_string(),
            repo: "org/repo".to_string(),
            status: PRStatus::Open,
            created_at: "2024-05-01T10:00:00Z".to_string(),
            updated_at: "2024-05-01T10:00:00Z".to_string(),
            additions: 10,
            deletions: 5,
            review_status: ReviewStatus::Pending,
            comment_count: 0,
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 0,
            ci_status: CIStatus::Passing,
            mergeable: MergeableStatus::Unknown,
            head_ref: "sha123".to_string(),
            body: "Body text".to_string(),
            url: "https://github.com/org/repo/pull/1".to_string(),
            requested_reviewers: vec![],
            reviewers: vec![],
            is_draft: false,
            matched_queries: Vec::new(),
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: AttentionState::default(),
        }
    }

    async fn create_test_app() -> App<TestBackend> {
        let github = Arc::new(MockGithubProvider::new());
        let mut state_repo = MockStateRepository::new();
        state_repo.expect_load_config_json().returning(|| Ok(None));
        state_repo.expect_save_config_json().returning(|_| Ok(())).times(..);
        state_repo.expect_load_state().returning(|| Ok(vec![]));
        state_repo.expect_load_archive().returning(|| Ok(vec![]));
        let state_repo = Arc::new(state_repo);
        let backend = TestBackend::new(80, 24);

        let config_dir = std::env::temp_dir();
        let data_dir = std::env::temp_dir();

        App::with_deps(github, state_repo, &config_dir, &data_dir, backend).unwrap()
    }

    #[tokio::test]
    async fn test_handle_normal_key_navigation() {
        let mut app = create_test_app().await;
        app.pr_list.set_prs(vec![create_test_pr(), create_test_pr()]);
        app.pr_list.set_selected_index(0);

        // Move down
        handle_key(&mut app, KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)).await;
        assert_eq!(app.pr_list.selected_index(), 1);

        // Move up
        handle_key(&mut app, KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE)).await;
        assert_eq!(app.pr_list.selected_index(), 0);
    }

    #[tokio::test]
    async fn test_handle_normal_key_modes() {
        let mut app = create_test_app().await;

        // Help
        handle_key(&mut app, KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)).await;
        assert_eq!(app.mode, AppMode::Help);
        handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).await;
        assert_eq!(app.mode, AppMode::Normal);

        // Search
        handle_key(&mut app, KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE)).await;
        assert_eq!(app.mode, AppMode::Search);
        handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).await;
        assert_eq!(app.mode, AppMode::Normal);

        // Settings via right arrow
        handle_key(&mut app, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)).await;
        assert_eq!(app.mode, AppMode::Archive);
        handle_key(&mut app, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)).await;
        assert_eq!(app.mode, AppMode::Settings);
    }

    #[tokio::test]
    async fn test_handle_search_input() {
        let mut app = create_test_app().await;
        app.mode = AppMode::Search;

        handle_key(&mut app, KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)).await;
        handle_key(&mut app, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)).await;
        assert_eq!(app.input_buffer, "ab");

        handle_key(&mut app, KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)).await;
        assert_eq!(app.input_buffer, "a");

        handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).await;
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[tokio::test]
    async fn test_theme_picker_enter_captures_original_and_sets_index() {
        let mut app = create_test_app().await;
        app.mode = AppMode::Settings;
        app.settings_selected_index = 5;
        app.config.theme = "nord".to_string();

        handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).await;

        assert_eq!(app.mode, AppMode::ThemePicker);
        assert_eq!(app.theme_picker_original, Some("nord".to_string()));
        let nord_idx =
            ratatui_themes::ThemeName::all().iter().position(|t| t.slug() == "nord").unwrap();
        assert_eq!(app.theme_picker_index, nord_idx);
    }

    #[tokio::test]
    async fn test_theme_picker_j_updates_index_and_live_preview() {
        let mut app = create_test_app().await;
        app.mode = AppMode::ThemePicker;
        app.theme_picker_index = 0;
        app.theme_picker_original = Some("dracula".to_string());
        app.config.theme = "dracula".to_string();

        handle_key(&mut app, KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)).await;

        assert_eq!(app.theme_picker_index, 1);
        let expected_slug = ratatui_themes::ThemeName::all()[1].slug();
        assert_eq!(app.config.theme, expected_slug);
    }

    #[tokio::test]
    async fn test_theme_picker_enter_commits_theme() {
        let mut app = create_test_app().await;
        app.mode = AppMode::ThemePicker;
        app.theme_picker_index = 2;
        app.theme_picker_original = Some("dracula".to_string());
        app.config.theme = "nord".to_string();

        handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).await;

        assert_eq!(app.mode, AppMode::Settings);
        assert_eq!(app.theme_picker_original, None);
        assert_eq!(app.config.theme, "nord");
    }

    #[tokio::test]
    async fn test_handle_settings_edit_query_prefills_buffers() {
        let mut app = create_test_app().await;
        app.mode = AppMode::Settings;
        app.config.queries = vec![crate::config::QueryConfig {
            name: "My Query".to_string(),
            search: "is:pr author:me".to_string(),
            interval: "60s".to_string(),
            enabled: true,
        }];
        app.settings_selected_index = QUERIES_START;

        handle_key(&mut app, KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE)).await;

        assert_eq!(app.mode, AppMode::AddQueryName);
        assert_eq!(app.editing_query_index, Some(0));
        assert_eq!(app.query_name_buffer, "My Query");
        assert_eq!(app.query_search_buffer, "is:pr author:me");
    }

    #[tokio::test]
    async fn test_handle_settings_delete_query_shows_confirm() {
        let mut app = create_test_app().await;
        app.mode = AppMode::Settings;
        app.config.queries = vec![crate::config::QueryConfig {
            name: "My Query".to_string(),
            search: "is:pr author:me".to_string(),
            interval: "60s".to_string(),
            enabled: true,
        }];
        app.settings_selected_index = QUERIES_START;

        handle_key(&mut app, KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)).await;

        assert_eq!(app.mode, AppMode::DeleteQueryConfirm);
        assert_eq!(app.deleting_query_index, Some(0));
    }

    #[tokio::test]
    async fn test_handle_delete_confirm_y_removes_query() {
        let mut app = create_test_app().await;
        app.mode = AppMode::DeleteQueryConfirm;
        app.config.queries = vec![crate::config::QueryConfig {
            name: "My Query".to_string(),
            search: "is:pr author:me".to_string(),
            interval: "60s".to_string(),
            enabled: true,
        }];
        app.deleting_query_index = Some(0);
        app.settings_selected_index = QUERIES_START;

        handle_key(&mut app, KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)).await;

        assert_eq!(app.mode, AppMode::Settings);
        assert!(app.config.queries.is_empty());
        assert_eq!(app.deleting_query_index, None);
    }

    #[tokio::test]
    async fn test_handle_delete_confirm_esc_cancels() {
        let mut app = create_test_app().await;
        app.mode = AppMode::DeleteQueryConfirm;
        app.config.queries = vec![crate::config::QueryConfig {
            name: "My Query".to_string(),
            search: "is:pr author:me".to_string(),
            interval: "60s".to_string(),
            enabled: true,
        }];
        app.deleting_query_index = Some(0);

        handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).await;

        assert_eq!(app.mode, AppMode::Settings);
        assert_eq!(app.config.queries.len(), 1);
        assert_eq!(app.deleting_query_index, None);
    }

    #[tokio::test]
    async fn test_handle_settings_edit_esc_from_name_clears_editing_index() {
        let mut app = create_test_app().await;
        app.mode = AppMode::AddQueryName;
        app.editing_query_index = Some(0);

        handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).await;

        assert_eq!(app.mode, AppMode::Settings);
        assert_eq!(app.editing_query_index, None);
    }

    #[tokio::test]
    async fn test_edit_max_age_days_enter_sets_value() {
        let mut app = create_test_app().await;
        app.mode = AppMode::EditMaxAgeDays;
        app.max_age_days_buffer = "14".to_string();

        handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).await;

        assert_eq!(app.mode, AppMode::Settings);
        assert_eq!(app.config.max_age_days, Some(14));
    }

    #[tokio::test]
    async fn test_edit_max_age_days_enter_empty_clears_value() {
        let mut app = create_test_app().await;
        app.config.max_age_days = Some(7);
        app.mode = AppMode::EditMaxAgeDays;
        app.max_age_days_buffer = String::new();

        handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).await;

        assert_eq!(app.mode, AppMode::Settings);
        assert_eq!(app.config.max_age_days, None);
    }

    #[tokio::test]
    async fn test_edit_max_age_days_esc_does_not_change_value() {
        let mut app = create_test_app().await;
        app.config.max_age_days = Some(7);
        app.mode = AppMode::EditMaxAgeDays;
        app.max_age_days_buffer = "99".to_string();

        handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).await;

        assert_eq!(app.mode, AppMode::Settings);
        assert_eq!(app.config.max_age_days, Some(7));
    }

    #[tokio::test]
    async fn test_edit_max_age_days_ignores_non_digit_chars() {
        let mut app = create_test_app().await;
        app.mode = AppMode::EditMaxAgeDays;
        app.max_age_days_buffer = String::new();

        handle_key(&mut app, KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)).await;
        handle_key(&mut app, KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE)).await;
        handle_key(&mut app, KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE)).await;

        assert_eq!(app.max_age_days_buffer, "14");
    }

    #[tokio::test]
    async fn test_handle_settings_enter_on_max_age_row_opens_edit_mode() {
        let mut app = create_test_app().await;
        app.mode = AppMode::Settings;
        app.config.max_age_days = Some(21);
        app.settings_selected_index = 2;

        handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).await;

        assert_eq!(app.mode, AppMode::EditMaxAgeDays);
        assert_eq!(app.max_age_days_buffer, "21");
    }

    #[tokio::test]
    async fn test_theme_picker_esc_restores_original_theme() {
        let mut app = create_test_app().await;
        app.mode = AppMode::ThemePicker;
        app.theme_picker_index = 2;
        app.theme_picker_original = Some("dracula".to_string());
        app.config.theme = "nord".to_string();

        handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).await;

        assert_eq!(app.mode, AppMode::Settings);
        assert_eq!(app.config.theme, "dracula");
        assert_eq!(app.theme_picker_original, None);
    }
}
