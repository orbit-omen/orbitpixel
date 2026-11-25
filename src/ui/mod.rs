use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, InputMode, RenderMode};

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tab bar
            Constraint::Length(3), // Address bar
            Constraint::Min(3),    // Viewport
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    render_tab_bar(frame, app, chunks[0]);
    render_address_bar(frame, app, chunks[1]);
    render_viewport(frame, app, chunks[2]);
    render_status_bar(frame, app, chunks[3]);
}

fn render_tab_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let mut spans = vec![Span::styled(
        " ORBITPIXEL ",
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];
    spans.push(Span::raw(" "));

    for (idx, tab) in app.tabs.iter().enumerate() {
        let short_title: String = tab.document.title.chars().take(18).collect();
        let label = format!(" [{}: {}] ", idx + 1, short_title);
        if idx == app.active_tab {
            spans.push(Span::styled(
                label,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(label, Style::default().fg(Color::DarkGray)));
        }
    }

    let block = Block::default().borders(Borders::ALL);
    frame.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
}

fn render_address_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let tab = app.current_tab();
    let back_ok = if tab.back_stack.is_empty() { " " } else { "←" };
    let fwd_ok = if tab.forward_stack.is_empty() { " " } else { "→" };
    let reload_icon = if tab.is_loading { "◌" } else { "↻" };

    let is_editing = app.input_mode == InputMode::InsertUrl;
    let url_style = if is_editing {
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let display_url = if is_editing {
        format!("{}▏", app.url_input)
    } else {
        app.url_input.clone()
    };

    let line = Line::from(vec![
        Span::styled(
            format!(" {} {} {}  ", back_ok, fwd_ok, reload_icon),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(display_url, url_style),
    ]);

    let border_style = if is_editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let title = if is_editing {
        " URL (Enter to go, Esc to cancel) "
    } else {
        " Address (Ctrl+L to edit, m to switch mode) "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn render_viewport(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let tab = app.current_tab();
    let block = Block::default()
        .title(format!(" {} ", tab.document.title))
        .borders(Borders::ALL);

    let paragraph = Paragraph::new(tab.rendered.lines.clone())
        .block(block)
        .scroll((tab.scroll_y, 0));

    frame.render_widget(paragraph, area);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let tab = app.current_tab();
    let mode_color = match app.mode {
        RenderMode::Text => Color::Gray,
        RenderMode::Color => Color::Green,
        RenderMode::Pixel => Color::Magenta,
    };

    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", app.mode.label()),
            Style::default()
                .fg(Color::Black)
                .bg(mode_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            " {} | {}x{} | Links: {} | q: Quit ",
            tab.status_text,
            area.width,
            area.height,
            tab.rendered.links.len()
        )),
    ]);

    frame.render_widget(
        Paragraph::new(line).style(Style::default().fg(Color::White).bg(Color::DarkGray)),
        area,
    );
}
