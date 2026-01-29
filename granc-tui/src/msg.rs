use crate::config::AppConfig;
use crate::model::MethodData;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum Msg {
    // --- User Input ---
    Key(KeyEvent),

    // --- Config Lifecycle ---
    ConfigLoaded(Result<AppConfig, String>),

    // --- Async Results ---
    ServicesFetched {
        project_id: Uuid,
        services: Vec<String>,
    },
    MethodsFetched {
        service: String,
        methods: Vec<MethodData>,
    },
    CallResponse(Result<String, String>),

    // --- System ---
    NoOp,
    Exit,
}

impl From<Event> for Msg {
    fn from(event: Event) -> Self {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('c')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    Msg::Exit
                }
                _ => Msg::Key(key),
            },
            _ => Msg::NoOp,
        }
    }
}
