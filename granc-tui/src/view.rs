use crate::model::{Focus, Model, Screen};
use color_eyre::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::ListState,
    widgets::{Block, Borders, List, ListItem, Paragraph, StatefulWidget, Widget, WidgetRef},
};
use teatui::View;

struct RootWidget {
    model: Model,
}

impl WidgetRef for RootWidget {
    fn render_ref(&self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        let main_area = chunks[0];
        let status_bar = chunks[1];

        match self.model.screen {
            Screen::Dashboard => draw_dashboard(&self.model, main_area, buf),
            Screen::NewProject => draw_new_project(&self.model, main_area, buf),
            Screen::ServiceBrowser => draw_services(&self.model, main_area, buf),
            Screen::MethodBrowser => draw_method_browser(&self.model, main_area, buf),
            Screen::MethodView => draw_method_execution(&self.model, main_area, buf),
            Screen::ResponseView => draw_response(&self.model, main_area, buf),
        }

        let msg = self.model.status_message.as_deref().unwrap_or("Ready");
        let status_text = format!(
            " {} | Screen: {:?} | [Q] Quit | [L] Load",
            msg, self.model.screen
        );
        Paragraph::new(status_text)
            .style(Style::default().bg(Color::Blue).fg(Color::White))
            .render_ref(status_bar, buf);
    }
}

pub fn view(model: &Model) -> Result<View> {
    Ok(View::new(RootWidget {
        model: model.clone(),
    }))
}

// --- Helpers ---

fn draw_dashboard(model: &Model, area: Rect, buf: &mut ratatui::prelude::Buffer) {
    let items: Vec<ListItem> = model
        .config
        .projects
        .iter()
        .map(|p| ListItem::new(p.name.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("Projects (Press 'n' for new)")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).bold());

    let mut state = ListState::default().with_selected(Some(model.project_list_idx));
    StatefulWidget::render(list, area, buf, &mut state);
}

fn draw_new_project(model: &Model, area: Rect, buf: &mut ratatui::prelude::Buffer) {
    let text = Paragraph::new(format!("Server URL: {}", model.input_buffer)).block(
        Block::default()
            .title("New Project (Enter URL)")
            .borders(Borders::ALL),
    );
    text.render_ref(area, buf);
}

fn draw_services(model: &Model, area: Rect, buf: &mut ratatui::prelude::Buffer) {
    let items: Vec<ListItem> = model
        .services
        .iter()
        .map(|s| ListItem::new(s.as_str()))
        .collect();

    let list = List::new(items)
        .block(Block::default().title("Services").borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::DarkGray).bold());

    let mut state = ListState::default().with_selected(Some(model.service_list_idx));
    StatefulWidget::render(list, area, buf, &mut state);
}

fn draw_method_browser(model: &Model, area: Rect, buf: &mut ratatui::prelude::Buffer) {
    let items: Vec<ListItem> = model
        .methods
        .iter()
        .map(|m| ListItem::new(m.signature.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(
                    "Methods of {}",
                    model.selected_service.as_deref().unwrap_or("?")
                ))
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).bold());

    let mut state = ListState::default().with_selected(Some(model.method_list_idx));
    StatefulWidget::render(list, area, buf, &mut state);
}

fn draw_method_execution(model: &Model, area: Rect, buf: &mut ratatui::prelude::Buffer) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),      // Title
            Constraint::Percentage(60), // Body
            Constraint::Percentage(30), // Headers
            Constraint::Length(1),      // Hint
        ])
        .split(area);

    let title = format!(
        "Executing: {}",
        model.selected_method.as_deref().unwrap_or("?")
    );
    Paragraph::new(title).bold().render_ref(chunks[0], buf);

    // Body Editor
    let mut editor = model.body_editor.clone();
    let body_block = Block::default()
        .borders(Borders::ALL)
        .title("Request Body (JSON)");

    if model.focus == Focus::Body {
        editor.set_style(Style::default());
        editor.set_block(body_block.border_style(Style::default().fg(Color::Yellow)));
    } else {
        editor.set_style(Style::default().fg(Color::DarkGray));
        editor.set_block(body_block);
    }

    editor.render(chunks[1], buf);

    // Headers
    let header_block = Block::default()
        .borders(Borders::ALL)
        .title("Headers (Ctrl+H to add)");
    header_block.render_ref(chunks[2], buf);

    let header_area = chunks[2].inner(ratatui::layout::Margin {
        horizontal: 1,
        vertical: 1,
    });

    let header_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(1); model.headers.len()])
        .split(header_area);

    for (i, header) in model.headers.iter().enumerate() {
        if i >= header_rows.len() {
            break;
        }

        let row_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Length(1),
                Constraint::Percentage(45),
            ])
            .split(header_rows[i]);

        let k_style = if model.focus == Focus::HeaderKey(i) {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        Paragraph::new(header.key.as_str())
            .style(k_style)
            .render_ref(row_chunks[0], buf);

        Paragraph::new(":").render_ref(row_chunks[1], buf);

        let v_style = if model.focus == Focus::HeaderValue(i) {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        Paragraph::new(header.value.as_str())
            .style(v_style)
            .render_ref(row_chunks[2], buf);
    }

    Paragraph::new(
        "[Tab] Cycle Focus | [Ctrl+Enter/S] Send | [Ctrl+H] Add Header | [Ctrl+D] Remove Header",
    )
    .style(Style::default().fg(Color::Gray))
    .render_ref(chunks[3], buf);
}

fn draw_response(model: &Model, area: Rect, buf: &mut ratatui::prelude::Buffer) {
    let p = Paragraph::new(model.response_output.as_str()).block(
        Block::default()
            .title("Response (Esc to back)")
            .borders(Borders::ALL),
    );
    p.render_ref(area, buf);
}
