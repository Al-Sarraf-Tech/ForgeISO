use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Row, Table, Wrap},
};

use crate::state::App;

pub(super) fn draw_build_step(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    if app.build_is_complete() {
        // Show result.
        let mut lines = vec![
            Line::styled(
                "ISO Ready",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from(""),
            Line::from("The guided workflow is complete."),
            Line::from("Optional checks are available if you want extra assurance."),
            Line::from(""),
        ];

        if let Some(ref art) = app.build_artifact {
            lines.push(Line::from(format!("Artifact: {}", art.display())));
        }
        if let Some(ref sha) = app.build_sha256 {
            lines.push(Line::from(format!("SHA-256:  {sha}")));
        }

        lines.push(Line::from(""));
        lines.push(Line::styled(
            "  c  Optional checks    o  Open folder    r  Rebuild    q  Quit",
            Style::default().fg(Color::DarkGray),
        ));

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Build Result "),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(para, area);
    } else if app.busy {
        // Show progress.
        let lines = vec![
            Line::styled(
                "Building...",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from(""),
            Line::from("Progress updates stream into the log panel below."),
        ];
        let para = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" Build "))
            .wrap(Wrap { trim: false });
        frame.render_widget(para, area);
    } else {
        // Show summary card.
        let summary = app.summary_lines();
        let rows: Vec<Row<'_>> = summary
            .iter()
            .map(|(label, value)| {
                Row::new(vec![
                    ratatui::text::Text::styled(
                        label.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    ratatui::text::Text::from(value.clone()),
                ])
            })
            .collect();

        let widths = [Constraint::Length(16), Constraint::Min(30)];
        let table = Table::new(rows, widths).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Review Build Plan — Press Enter to create the ISO "),
        );
        frame.render_widget(table, area);
    }
}
