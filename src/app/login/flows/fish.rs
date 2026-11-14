// SPDX-License-Identifier: MPL-2.0

use super::super::{LoginEventKind, LoginFlow, apply_login_success_checked};
use crate::account_selection::select_account_after_login;
use crate::app::{AppModel, Config, Handle, Message, ProviderId, Task};
use crate::providers::fish;
use crate::providers::fish::login::{FishLoginEvent, FishLoginState, FishLoginStatus};

pub(crate) struct FishLoginFlow;

impl LoginFlow for FishLoginFlow {
    type State = FishLoginState;
    type Event = FishLoginEvent;
    const PROVIDER: ProviderId = ProviderId::Fish;

    fn state(app: &AppModel) -> &Option<Self::State> {
        &app.fish_login
    }

    fn state_mut(app: &mut AppModel) -> &mut Option<Self::State> {
        &mut app.fish_login
    }

    fn handle_mut(app: &mut AppModel) -> &mut Option<Handle> {
        &mut app.fish_login_handle
    }

    fn is_running(state: &Self::State) -> bool {
        state.status == FishLoginStatus::Editing
    }

    fn log_id(state: &Self::State) -> &str {
        &state.account_id
    }

    fn status_debug(state: &Self::State) -> String {
        format!("{:?}", state.status)
    }

    fn account_exists(config: &Config, account_id: &str) -> bool {
        config
            .fish_managed_accounts
            .iter()
            .any(|account| account.id == account_id)
    }

    fn failed_state(error: String) -> Self::State {
        FishLoginState::failed(error)
    }

    fn prepare(config: Config) -> Result<(Self::State, cosmic::iced::Task<Self::Event>), String> {
        let _ = config;
        Ok((fish::login::prepare(), cosmic::iced::Task::none()))
    }

    fn prepare_for_reauth(
        config: Config,
        account_id: &str,
    ) -> Result<(Self::State, cosmic::iced::Task<Self::Event>), String> {
        Ok((
            fish::login::prepare_for_reauth(config, account_id)?,
            cosmic::iced::Task::none(),
        ))
    }

    fn wrap_event(event: Self::Event) -> Message {
        Message::LoginEvent(ProviderId::Fish, Box::new(LoginEventKind::Fish(event)))
    }

    fn on_event(app: &mut AppModel, event: Self::Event) -> Task<Message> {
        match event {
            FishLoginEvent::ApiKeyChanged(api_key) => {
                if let Some(login) = app.fish_login.as_mut() {
                    login.update_api_key(api_key);
                }
                Task::none()
            }
            FishLoginEvent::LabelChanged(label) => {
                if let Some(login) = app.fish_login.as_mut() {
                    login.update_label(label);
                }
                Task::none()
            }
            FishLoginEvent::ApiKeyVisibilityToggled => {
                if let Some(login) = app.fish_login.as_mut() {
                    login.toggle_api_key_visibility();
                }
                Task::none()
            }
            FishLoginEvent::Saved => {
                let Some(login) = app.fish_login.as_mut() else {
                    return Task::none();
                };
                let flow_id = login.account_id.clone();
                let is_reauthentication = app
                    .config
                    .fish_managed_accounts
                    .iter()
                    .any(|account| account.id == flow_id);
                match fish::login::save(&app.config, login) {
                    Ok(managed_account) => {
                        let account_id = managed_account.id.clone();
                        let selected_account_id = account_id.clone();
                        let result = apply_login_success_checked(
                            app,
                            ProviderId::Fish,
                            &flow_id,
                            account_id.clone(),
                            move |config| {
                                fish::account::apply_login_account(config, managed_account);
                                select_account_after_login(
                                    config,
                                    ProviderId::Fish,
                                    selected_account_id,
                                );
                            },
                        );
                        match result {
                            Ok(task) => {
                                app.fish_login_handle = None;
                                app.fish_login = None;
                                task
                            }
                            Err(()) => {
                                cleanup_new_account_storage(&account_id, is_reauthentication);
                                if let Some(login) = app.fish_login.as_mut() {
                                    login.status = FishLoginStatus::Editing;
                                    login.error = Some(
                                        "Failed to save Fish Audio account configuration"
                                            .to_string(),
                                    );
                                }
                                Task::none()
                            }
                        }
                    }
                    Err(_) => Task::none(),
                }
            }
        }
    }
}

fn cleanup_new_account_storage(account_id: &str, is_reauthentication: bool) {
    if is_reauthentication {
        return;
    }
    if let Err(error) = fish::storage::delete_account(account_id) {
        tracing::error!(
            account_id,
            error = %error,
            "failed to clean up new Fish Audio account storage"
        );
    }
}
