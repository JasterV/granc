use color_eyre::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use teatui::Update;
use uuid::Uuid;

use crate::config::{ConnectionConfig, Project};
use crate::effects::Effect;
use crate::model::{Focus, HeaderPair, Model, Screen};
use crate::msg::Msg;

pub fn update(mut model: Model, msg: Msg) -> Result<Update<Model, Effect>> {
    match msg {
        Msg::Exit => Ok(Update::Exit),
        Msg::NoOp => Ok(Update::Next(model)),

        Msg::ConfigLoaded(res) => {
            match res {
                Ok(cfg) => {
                    model.config = cfg;
                    model.status_message = Some("Config loaded".into());
                }
                Err(e) => model.status_message = Some(format!("Config Error: {}", e)),
            }
            Ok(Update::Next(model))
        }

        // Handle Editor Specific Messages (Method View)
        Msg::Key(key) if model.screen == Screen::MethodView => match key.code {
            KeyCode::Esc => {
                model.screen = Screen::MethodBrowser;
                Ok(Update::Next(model))
            }
            KeyCode::Tab => {
                match model.focus {
                    Focus::Body => {
                        if !model.headers.is_empty() {
                            model.focus = Focus::HeaderKey(0);
                        }
                    }
                    Focus::HeaderKey(i) => model.focus = Focus::HeaderValue(i),
                    Focus::HeaderValue(i) => {
                        if i + 1 < model.headers.len() {
                            model.focus = Focus::HeaderKey(i + 1);
                        } else {
                            model.focus = Focus::Body;
                        }
                    }
                }
                Ok(Update::Next(model))
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                model.headers.push(HeaderPair {
                    key: "".into(),
                    value: "".into(),
                });
                model.focus = Focus::HeaderKey(model.headers.len() - 1);
                Ok(Update::Next(model))
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                match model.focus {
                    Focus::HeaderKey(i) | Focus::HeaderValue(i) => {
                        model.headers.remove(i);
                        model.focus = Focus::Body;
                    }
                    _ => {}
                }
                Ok(Update::Next(model))
            }
            KeyCode::Enter | KeyCode::Char('s')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                execute_request(model)
            }
            _ => {
                match model.focus {
                    Focus::Body => {
                        model.body_editor.input(to_ratatui_key(key));
                    }
                    Focus::HeaderKey(i) => {
                        handle_text_input(&mut model.headers[i].key, key);
                    }
                    Focus::HeaderValue(i) => {
                        handle_text_input(&mut model.headers[i].value, key);
                    }
                }
                Ok(Update::Next(model))
            }
        },

        Msg::Key(key) => match (model.screen.clone(), key.code) {
            // Global
            (_, KeyCode::Char('q')) => Ok(Update::Exit),

            // --- Dashboard ---
            (Screen::Dashboard, KeyCode::Char('l')) => {
                Ok(Update::NextWithEffect(model, Effect::LoadConfigFromDisk))
            }
            (Screen::Dashboard, KeyCode::Char('n')) => {
                model.screen = Screen::NewProject;
                model.input_buffer.clear();
                Ok(Update::Next(model))
            }
            (Screen::Dashboard, KeyCode::Down) => {
                if !model.config.projects.is_empty() {
                    model.project_list_idx =
                        (model.project_list_idx + 1) % model.config.projects.len();
                }
                Ok(Update::Next(model))
            }
            (Screen::Dashboard, KeyCode::Up) => {
                if !model.config.projects.is_empty() {
                    model.project_list_idx = if model.project_list_idx == 0 {
                        model.config.projects.len() - 1
                    } else {
                        model.project_list_idx - 1
                    };
                }
                Ok(Update::Next(model))
            }
            (Screen::Dashboard, KeyCode::Enter) => {
                let project_opt = model.config.projects.get(model.project_list_idx).cloned();
                if let Some(proj) = project_opt {
                    model.selected_project_id = Some(proj.id);
                    model.screen = Screen::ServiceBrowser;
                    model.service_list_idx = 0;
                    model.status_message = Some("Fetching services...".into());
                    Ok(Update::NextWithEffect(model, Effect::FetchServices(proj)))
                } else {
                    Ok(Update::Next(model))
                }
            }

            // --- New Project ---
            (Screen::NewProject, KeyCode::Enter) => {
                let new_proj = Project {
                    id: Uuid::new_v4(),
                    name: if model.input_buffer.is_empty() {
                        "Untitled".to_string()
                    } else {
                        model.input_buffer.clone()
                    },
                    connection: ConnectionConfig::Reflection {
                        url: model.input_buffer.clone(),
                    },
                    saved_requests: vec![],
                };
                model.config.projects.push(new_proj);
                model.screen = Screen::Dashboard;
                model.project_list_idx = model.config.projects.len() - 1;
                let effect = Effect::SaveConfigToDisk(model.config.clone());
                Ok(Update::NextWithEffect(model, effect))
            }
            (Screen::NewProject, KeyCode::Char(c)) => {
                model.input_buffer.push(c);
                Ok(Update::Next(model))
            }
            (Screen::NewProject, KeyCode::Backspace) => {
                model.input_buffer.pop();
                Ok(Update::Next(model))
            }
            (Screen::NewProject, KeyCode::Esc) => {
                model.screen = Screen::Dashboard;
                Ok(Update::Next(model))
            }

            // --- Service Browser ---
            (Screen::ServiceBrowser, KeyCode::Down) => {
                if !model.services.is_empty() {
                    model.service_list_idx = (model.service_list_idx + 1) % model.services.len();
                }
                Ok(Update::Next(model))
            }
            (Screen::ServiceBrowser, KeyCode::Up) => {
                if !model.services.is_empty() {
                    model.service_list_idx = if model.service_list_idx == 0 {
                        model.services.len() - 1
                    } else {
                        model.service_list_idx - 1
                    };
                }
                Ok(Update::Next(model))
            }
            (Screen::ServiceBrowser, KeyCode::Enter) => {
                if let (Some(svc), Some(proj)) = (
                    model.services.get(model.service_list_idx).cloned(),
                    model.current_project().cloned(),
                ) {
                    model.selected_service = Some(svc.clone());
                    model.status_message = Some("Fetching methods...".into());
                    Ok(Update::NextWithEffect(
                        model,
                        Effect::FetchMethods {
                            project: proj,
                            service: svc,
                        },
                    ))
                } else {
                    Ok(Update::Next(model))
                }
            }
            (Screen::ServiceBrowser, KeyCode::Esc) => {
                model.screen = Screen::Dashboard;
                Ok(Update::Next(model))
            }

            // --- Method Browser ---
            (Screen::MethodBrowser, KeyCode::Down) => {
                if !model.methods.is_empty() {
                    model.method_list_idx = (model.method_list_idx + 1) % model.methods.len();
                }
                Ok(Update::Next(model))
            }
            (Screen::MethodBrowser, KeyCode::Up) => {
                if !model.methods.is_empty() {
                    model.method_list_idx = if model.method_list_idx == 0 {
                        model.methods.len() - 1
                    } else {
                        model.method_list_idx - 1
                    };
                }
                Ok(Update::Next(model))
            }
            (Screen::MethodBrowser, KeyCode::Enter) => {
                if let Some(m) = model.methods.get(model.method_list_idx).cloned() {
                    model.selected_method = Some(m.name);
                    model.headers.clear();
                    model.focus = Focus::Body;
                    model.screen = Screen::MethodView;
                }
                Ok(Update::Next(model))
            }
            (Screen::MethodBrowser, KeyCode::Esc) => {
                model.screen = Screen::ServiceBrowser;
                Ok(Update::Next(model))
            }

            // --- Response View ---
            (Screen::ResponseView, KeyCode::Esc) => {
                model.screen = Screen::MethodView;
                Ok(Update::Next(model))
            }

            _ => Ok(Update::Next(model)),
        },

        Msg::ServicesFetched {
            project_id,
            services,
        } => {
            if model.selected_project_id == Some(project_id) {
                model.services = services;
                model.service_list_idx = 0;
                model.status_message = Some("Services loaded.".into());
            }
            Ok(Update::Next(model))
        }

        Msg::MethodsFetched { service, methods } => {
            if model.selected_service.as_deref() == Some(&service) {
                model.methods = methods;
                model.method_list_idx = 0;
                model.screen = Screen::MethodBrowser;
                model.status_message = Some("Methods loaded.".into());
            }
            Ok(Update::Next(model))
        }

        Msg::CallResponse(res) => {
            model.screen = Screen::ResponseView;
            match res {
                Ok(s) => {
                    model.response_output = s;
                    model.status_message = Some("Call success".into());
                }
                Err(e) => {
                    model.response_output = format!("Error: {}", e);
                    model.status_message = Some("Call failed".into());
                }
            }
            Ok(Update::Next(model))
        }
    }
}

fn execute_request(mut model: Model) -> Result<Update<Model, Effect>> {
    let execution_data = if let (Some(p), Some(s), Some(m)) = (
        model.current_project(),
        &model.selected_service,
        &model.selected_method,
    ) {
        Some((p.clone(), s.clone(), m.clone()))
    } else {
        None
    };

    if let Some((p, s, m)) = execution_data {
        let headers: Vec<(String, String)> = model
            .headers
            .iter()
            .filter(|h| !h.key.is_empty())
            .map(|h| (h.key.clone(), h.value.clone()))
            .collect();

        let body_lines = model.body_editor.lines().to_vec();
        let body = body_lines.join("\n");

        let effect = Effect::ExecuteCall {
            project: p,
            service: s,
            method: m,
            body,
            headers,
        };
        model.status_message = Some("Executing request...".into());
        Ok(Update::NextWithEffect(model, effect))
    } else {
        model.status_message =
            Some("Error: Missing execution context (project/service/method)".into());
        Ok(Update::Next(model))
    }
}

fn handle_text_input(target: &mut String, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Char(c) => target.push(c),
        KeyCode::Backspace => {
            target.pop();
        }
        _ => {}
    }
}

fn to_ratatui_key(key: crossterm::event::KeyEvent) -> ratatui::crossterm::event::KeyEvent {
    let code = match key.code {
        crossterm::event::KeyCode::Backspace => ratatui::crossterm::event::KeyCode::Backspace,
        crossterm::event::KeyCode::Enter => ratatui::crossterm::event::KeyCode::Enter,
        crossterm::event::KeyCode::Left => ratatui::crossterm::event::KeyCode::Left,
        crossterm::event::KeyCode::Right => ratatui::crossterm::event::KeyCode::Right,
        crossterm::event::KeyCode::Up => ratatui::crossterm::event::KeyCode::Up,
        crossterm::event::KeyCode::Down => ratatui::crossterm::event::KeyCode::Down,
        crossterm::event::KeyCode::Home => ratatui::crossterm::event::KeyCode::Home,
        crossterm::event::KeyCode::End => ratatui::crossterm::event::KeyCode::End,
        crossterm::event::KeyCode::PageUp => ratatui::crossterm::event::KeyCode::PageUp,
        crossterm::event::KeyCode::PageDown => ratatui::crossterm::event::KeyCode::PageDown,
        crossterm::event::KeyCode::Tab => ratatui::crossterm::event::KeyCode::Tab,
        crossterm::event::KeyCode::BackTab => ratatui::crossterm::event::KeyCode::BackTab,
        crossterm::event::KeyCode::Delete => ratatui::crossterm::event::KeyCode::Delete,
        crossterm::event::KeyCode::Insert => ratatui::crossterm::event::KeyCode::Insert,
        crossterm::event::KeyCode::F(n) => ratatui::crossterm::event::KeyCode::F(n),
        crossterm::event::KeyCode::Char(c) => ratatui::crossterm::event::KeyCode::Char(c),
        crossterm::event::KeyCode::Null => ratatui::crossterm::event::KeyCode::Null,
        crossterm::event::KeyCode::Esc => ratatui::crossterm::event::KeyCode::Esc,
        _ => ratatui::crossterm::event::KeyCode::Null,
    };

    let modifiers =
        ratatui::crossterm::event::KeyModifiers::from_bits_truncate(key.modifiers.bits());

    ratatui::crossterm::event::KeyEvent {
        code,
        modifiers,
        kind: ratatui::crossterm::event::KeyEventKind::Press,
        state: ratatui::crossterm::event::KeyEventState::empty(),
    }
}
