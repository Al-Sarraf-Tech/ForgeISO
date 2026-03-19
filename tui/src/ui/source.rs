use forgeiso_engine::all_presets;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::state::{App, SourceFocus};

pub(super) fn draw_source_step(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),    // preset list
            Constraint::Length(3), // manual input
            Constraint::Length(3), // detected info
        ])
        .split(area);

    // Preset list.
    let presets = all_presets();
    let border_style = if app.source_focus == SourceFocus::PresetList {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    // Calculate scroll offset for the list to keep the cursor visible.
    let visible_height = chunks[0].height.saturating_sub(2) as usize;
    let list_offset = if app.preset_scroll >= visible_height {
        app.preset_scroll - visible_height + 1
    } else {
        0
    };

    // Render the list items with manual scroll offset.
    let visible_items: Vec<ListItem<'_>> = presets
        .iter()
        .enumerate()
        .skip(list_offset)
        .take(visible_height)
        .map(|(i, p)| {
            let marker = if app.preset_selected == Some(i) {
                ">"
            } else {
                " "
            };
            let style = if i == app.preset_scroll {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if app.preset_selected == Some(i) {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            ListItem::new(Line::styled(
                format!("{marker} {:<30} {:<12} {}", p.name, p.distro, p.note),
                style,
            ))
        })
        .collect();

    let scrolled_list = List::new(visible_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(format!(
                " Presets ({}/{}) ",
                app.preset_scroll + 1,
                presets.len()
            )),
    );
    frame.render_widget(scrolled_list, chunks[0]);

    // Manual input.
    let input_border = if app.source_focus == SourceFocus::ManualInput {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let cursor = if app.source_focus == SourceFocus::ManualInput {
        "_"
    } else {
        ""
    };
    let input = Paragraph::new(Line::from(format!("{}{}", app.manual_source, cursor))).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(input_border)
            .title(" Manual Path/URL (Tab to focus) "),
    );
    frame.render_widget(input, chunks[1]);

    // Detected info.
    let info_text = if let Some(ref d) = app.detected_distro {
        format!("Detected: {d}")
    } else if !app.effective_source().is_empty() {
        "Source set (distro will be auto-detected)".into()
    } else {
        "No source selected".into()
    };
    let info_style = if app.detected_distro.is_some() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let info = Paragraph::new(Line::styled(info_text, info_style))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(info, chunks[2]);
}
