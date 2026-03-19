mod build;
mod check;
mod configure;
mod source;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Tabs},
};

use crate::state::{App, LogLevel, WizardStep};

pub(crate) fn ui(frame: &mut ratatui::Frame<'_>, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header / step indicator
            Constraint::Min(10),    // main content
            Constraint::Length(3),  // status bar
            Constraint::Length(10), // log panel
            Constraint::Length(3),  // help bar
        ])
        .split(frame.area());

    draw_header(frame, app, outer[0]);
    draw_main(frame, app, outer[1]);
    draw_status(frame, app, outer[2]);
    draw_log_panel(frame, app, outer[3]);
    draw_help_bar(frame, app, outer[4]);
}

fn draw_header(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let titles = WizardStep::ALL
        .iter()
        .map(|step| {
            let style = if *step == app.step {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if app.progress.step_complete(*step) {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::styled(step.label(), style)
        })
        .collect::<Vec<_>>();

    let tabs = Tabs::new(titles)
        .select(app.step.index())
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL).title(format!(
            " ForgeISO — Step {} of {}: {} ",
            app.step.one_based(),
            WizardStep::ALL.len(),
            app.step.label()
        )));
    frame.render_widget(tabs, area);
}

fn draw_main(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    match app.step {
        WizardStep::Source => source::draw_source_step(frame, app, area),
        WizardStep::Configure => configure::draw_configure_step(frame, app, area),
        WizardStep::Build => build::draw_build_step(frame, app, area),
        WizardStep::OptionalChecks => check::draw_check_step(frame, app, area),
    }
}

fn draw_status(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let style = if app.status.starts_with("Error") {
        Style::default().fg(Color::Red)
    } else if app.status.starts_with("Validation") || app.busy {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let busy_indicator = if app.busy { " [busy]" } else { "" };
    let para = Paragraph::new(Line::styled(
        format!("{}{}", app.status, busy_indicator),
        style,
    ))
    .block(Block::default().borders(Borders::ALL).title(" Status "));
    frame.render_widget(para, area);
}

fn draw_log_panel(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let visible = area.height.saturating_sub(2) as usize;
    let total = app.logs.len();
    let start = total.saturating_sub(visible);

    let lines: Vec<Line<'_>> = app
        .logs
        .iter()
        .skip(start)
        .take(visible)
        .map(|entry| {
            let style = match entry.level {
                LogLevel::Info => Style::default().fg(Color::Gray),
                LogLevel::Warn => Style::default().fg(Color::Yellow),
                LogLevel::Error => Style::default().fg(Color::Red),
            };
            Line::styled(entry.text.clone(), style)
        })
        .collect();

    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Log ({total} entries) ")),
    );
    frame.render_widget(para, area);
}

fn draw_help_bar(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let help_text = help_text_for_step(app.step, app.busy, app.build_is_complete());

    let para = Paragraph::new(Line::styled(
        help_text,
        Style::default().fg(Color::DarkGray),
    ))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(para, area);
}

pub(crate) fn help_text_for_step(
    step: WizardStep,
    busy: bool,
    build_complete: bool,
) -> &'static str {
    match step {
        WizardStep::Source => {
            "Tab: switch focus | Up/Down: browse | Enter: select | Right/n: next | q: quit"
        }
        WizardStep::Configure => {
            "Tab: sections | Up/Down: fields | Enter: edit | Space: toggle | Left/b: back | Right/n: next | q: quit"
        }
        WizardStep::Build if busy => "Building... please wait | q: quit",
        WizardStep::Build if build_complete => {
            "c: optional checks | o: open folder | r: rebuild | Right/n: optional checks | q: quit"
        }
        WizardStep::Build => "Enter: start build | Left/b: back | q: quit",
        WizardStep::OptionalChecks => {
            "Up/Down: fields | Enter: run action | Left/b: back to build | q: quit"
        }
    }
}
