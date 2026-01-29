use crate::config::{AppConfig, Project};
use tui_textarea::TextArea;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Dashboard,
    NewProject,
    ServiceBrowser,
    MethodBrowser,
    MethodView,
    ResponseView,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Body,
    HeaderKey(usize),
    HeaderValue(usize),
}

#[derive(Debug, Clone)]
pub struct MethodData {
    pub name: String,
    pub signature: String,
}

#[derive(Debug, Clone)]
pub struct HeaderPair {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub screen: Screen,
    pub config: AppConfig,

    // Global Input Buffer (New Project / URL)
    pub input_buffer: String,

    // Navigation Indices
    pub project_list_idx: usize,
    pub service_list_idx: usize,
    pub method_list_idx: usize,

    // Data
    pub selected_project_id: Option<Uuid>,
    pub services: Vec<String>,
    pub methods: Vec<MethodData>, // Updated to hold signature
    pub selected_service: Option<String>,
    pub selected_method: Option<String>,

    // Request Editor State
    pub body_editor: TextArea<'static>,
    pub headers: Vec<HeaderPair>,
    pub focus: Focus,

    // Results
    pub response_output: String,
    pub status_message: Option<String>,
}

impl Default for Model {
    fn default() -> Self {
        let mut editor = TextArea::default();
        editor.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title("Body (JSON)"),
        );
        editor.insert_str("{}");

        Self {
            screen: Screen::Dashboard,
            config: AppConfig::default(),
            input_buffer: String::new(),
            project_list_idx: 0,
            service_list_idx: 0,
            method_list_idx: 0,
            selected_project_id: None,
            services: vec![],
            methods: vec![],
            selected_service: None,
            selected_method: None,
            body_editor: editor,
            headers: vec![],
            focus: Focus::Body,
            response_output: String::new(),
            status_message: None,
        }
    }
}

impl Model {
    pub fn current_project(&self) -> Option<&Project> {
        self.selected_project_id
            .and_then(|id| self.config.projects.iter().find(|p| p.id == id))
    }
}
