use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Row, Table, Tabs},
};

use crate::state::{App, ConfigTab, FieldKind};

pub(super) fn draw_configure_step(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(5),    // field table
        ])
        .split(area);

    // Tab bar.
    let tab_titles: Vec<Line<'_>> = ConfigTab::ALL
        .iter()
        .map(|t| {
            let style = if *t == app.config_tab {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::styled(t.label(), style)
        })
        .collect();
    let tabs = Tabs::new(tab_titles)
        .select(app.config_tab.index())
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Tab to cycle sections "),
        );
    frame.render_widget(tabs, chunks[0]);

    // Fields table.
    let fields = app.tab_fields();
    let rows: Vec<Row<'_>> = fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let label_style = if i == app.field_index {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let value_display = f.display_value();
            let value_text = if i == app.field_index && app.editing {
                format!("{value_display}_")
            } else {
                value_display
            };

            let value_style = if f.is_toggle() {
                match f.kind {
                    FieldKind::Toggle(true) => Style::default().fg(Color::Green),
                    FieldKind::Toggle(false) => Style::default().fg(Color::DarkGray),
                    _ => Style::default(),
                }
            } else if i == app.field_index && app.editing {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            let indicator = if i == app.field_index { "> " } else { "  " };
            Row::new(vec![
                ratatui::text::Text::styled(format!("{indicator}{}", f.label), label_style),
                ratatui::text::Text::styled(value_text, value_style),
            ])
        })
        .collect();

    let widths = [Constraint::Length(24), Constraint::Min(30)];
    let table = Table::new(rows, widths).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", app.config_tab.label())),
    );
    frame.render_widget(table, chunks[1]);
}
