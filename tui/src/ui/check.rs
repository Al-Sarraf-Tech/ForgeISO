use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::state::App;

pub(super) fn draw_check_step(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // intro
            Constraint::Length(3), // verify source input
            Constraint::Length(7), // verify result
            Constraint::Length(9), // iso9660 result
            Constraint::Min(1),    // spacer
        ])
        .split(area);

    let intro = Paragraph::new(vec![
        Line::styled(
            "Optional Checks",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from("Your ISO is already built. Run checksum or ISO-9660 checks only if you want extra confidence."),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Step 4 "))
    .wrap(Wrap { trim: false });
    frame.render_widget(intro, chunks[0]);

    // Source input.
    let input_style = if app.check_field_index == 0 {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let cursor = if app.check_editing { "_" } else { "" };
    let input = Paragraph::new(Line::from(format!("{}{}", app.verify_source, cursor))).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(input_style)
            .title(" ISO Path "),
    );
    frame.render_widget(input, chunks[1]);

    // Verify result.
    let verify_lines = if let Some(ref r) = app.verify_result {
        let status_style = if r.matched {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Red)
        };
        vec![
            Line::styled(
                if r.matched { "PASS" } else { "FAIL" },
                status_style.add_modifier(Modifier::BOLD),
            ),
            Line::from(format!("File:     {}", r.filename)),
            Line::from(format!("Expected: {}", r.expected)),
            Line::from(format!("Actual:   {}", r.actual)),
        ]
    } else {
        let highlight = if app.check_field_index == 1 {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        vec![Line::styled("Press Enter to verify checksum", highlight)]
    };
    let verify_block = Paragraph::new(verify_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Checksum Verification "),
    );
    frame.render_widget(verify_block, chunks[2]);

    // ISO-9660 result.
    let iso_lines = if let Some(ref r) = app.iso9660_result {
        let status_style = if r.compliant {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Red)
        };
        vec![
            Line::styled(
                if r.compliant {
                    "ISO-9660 COMPLIANT"
                } else {
                    "NOT COMPLIANT"
                },
                status_style.add_modifier(Modifier::BOLD),
            ),
            Line::from(format!(
                "Volume ID: {}",
                r.volume_id.as_deref().unwrap_or("(none)")
            )),
            Line::from(format!("Size:      {} bytes", r.size_bytes)),
            Line::from(format!(
                "BIOS boot: {}",
                if r.boot_bios { "yes" } else { "no" }
            )),
            Line::from(format!(
                "UEFI boot: {}",
                if r.boot_uefi { "yes" } else { "no" }
            )),
        ]
    } else {
        let highlight = if app.check_field_index == 2 {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        vec![Line::styled("Press Enter to validate ISO-9660", highlight)]
    };
    let iso_block = Paragraph::new(iso_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" ISO-9660 Validation "),
    );
    frame.render_widget(iso_block, chunks[3]);
}
