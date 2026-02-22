use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Screen, ScrollFocus, SetupFocus, VerifierStatus};

pub fn draw(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Setup => draw_setup(frame, app),
        Screen::Running | Screen::Done => draw_running(frame, app),
    }
}

fn draw_setup(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Min(6),    // Prompt input
            Constraint::Length(3), // Verifier name input
            Constraint::Length(3), // Verifier prompt input
            Constraint::Min(4),    // Verifier list
            Constraint::Length(3), // Help bar
        ])
        .split(area);

    // Title
    let title = Paragraph::new("Verifiers TUI")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // Prompt input
    let prompt_style = if app.setup_focus == SetupFocus::Prompt {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };
    let prompt_block = Block::default()
        .title(" Prompt (what the worker should do) ")
        .borders(Borders::ALL)
        .border_style(prompt_style);
    let prompt_text = Paragraph::new(app.prompt_input.as_str())
        .block(prompt_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(prompt_text, chunks[1]);

    // Verifier name input
    let name_style = if app.setup_focus == SetupFocus::VerifierName {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };
    let name_block = Block::default()
        .title(" Verifier Name ")
        .borders(Borders::ALL)
        .border_style(name_style);
    let name_text = Paragraph::new(app.verifier_name_input.as_str()).block(name_block);
    frame.render_widget(name_text, chunks[2]);

    // Verifier prompt input
    let vprompt_style = if app.setup_focus == SetupFocus::VerifierPrompt {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };
    let vprompt_block = Block::default()
        .title(" Verifier Prompt ")
        .borders(Borders::ALL)
        .border_style(vprompt_style);
    let vprompt_text =
        Paragraph::new(app.verifier_prompt_input.as_str()).block(vprompt_block);
    frame.render_widget(vprompt_text, chunks[3]);

    // Verifier list
    let items: Vec<ListItem> = app
        .verifiers
        .iter()
        .enumerate()
        .map(|(i, v)| {
            ListItem::new(format!("  {}. {} — {}", i + 1, v.name, v.prompt))
        })
        .collect();
    let verifier_list = List::new(items).block(
        Block::default()
            .title(format!(" Verifiers ({}) ", app.verifiers.len()))
            .borders(Borders::ALL),
    );
    frame.render_widget(verifier_list, chunks[4]);

    // Help bar
    let can_start = app.can_start();
    let start_hint = if can_start {
        Span::styled(" Ctrl+S: Start ", Style::default().fg(Color::Green))
    } else {
        Span::styled(
            " Ctrl+S: Start (need prompt + verifier) ",
            Style::default().fg(Color::DarkGray),
        )
    };
    let help = Line::from(vec![
        Span::styled(" Tab: Next field ", Style::default().fg(Color::Cyan)),
        Span::raw(" | "),
        Span::styled(" Enter: Add verifier ", Style::default().fg(Color::Cyan)),
        Span::raw(" | "),
        Span::styled(" Ctrl+D: Remove last verifier ", Style::default().fg(Color::Cyan)),
        Span::raw(" | "),
        start_hint,
        Span::raw(" | "),
        Span::styled(" Ctrl+C/q: Quit ", Style::default().fg(Color::Red)),
    ]);
    let help_bar = Paragraph::new(help).block(Block::default().borders(Borders::TOP));
    frame.render_widget(help_bar, chunks[5]);

    // Show cursor in the focused input
    match app.setup_focus {
        SetupFocus::Prompt => {
            // Position cursor at end of prompt input, accounting for newlines and wrapping
            let inner_width = chunks[1].width.saturating_sub(2) as usize;
            if inner_width > 0 {
                let lines: Vec<&str> = app.prompt_input.split('\n').collect();
                let mut y_offset: usize = 0;

                // Count rows from all lines except the last
                for line in &lines[..lines.len().saturating_sub(1)] {
                    y_offset += (line.len() / inner_width) + 1;
                }

                // Position within the last line
                let last_line_len = lines.last().map_or(0, |l| l.len());
                y_offset += last_line_len / inner_width;
                let x_offset = last_line_len % inner_width;

                let x = chunks[1].x + 1 + x_offset as u16;
                let y = chunks[1].y + 1 + y_offset as u16;
                frame.set_cursor_position((x, y));
            }
        }
        SetupFocus::VerifierName => {
            let x = chunks[2].x + 1 + app.verifier_name_input.len() as u16;
            let y = chunks[2].y + 1;
            frame.set_cursor_position((x, y));
        }
        SetupFocus::VerifierPrompt => {
            let x = chunks[3].x + 1 + app.verifier_prompt_input.len() as u16;
            let y = chunks[3].y + 1;
            frame.set_cursor_position((x, y));
        }
    }
}

fn draw_running(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),                   // Title + status
            Constraint::Length(app.verifier_statuses.len() as u16 + 2), // Verifier checklist
            Constraint::Percentage(40),              // Logs
            Constraint::Percentage(40),              // File contents
            Constraint::Length(1),                   // Help bar
        ])
        .split(area);

    // Title + status
    let status_text = match app.screen {
        Screen::Done => "DONE - All verifiers passed!",
        _ => "Working...",
    };
    let status_color = match app.screen {
        Screen::Done => Color::Green,
        _ => Color::Yellow,
    };
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "Verifiers",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("Status: {}  Iteration: {}", status_text, app.iteration),
            Style::default().fg(status_color),
        ),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // Verifier checklist
    let verifier_items: Vec<ListItem> = app
        .verifier_statuses
        .iter()
        .map(|(name, status)| {
            let (icon, color) = match status {
                VerifierStatus::Pending => ("  ", Color::DarkGray),
                VerifierStatus::Running => (">>", Color::Yellow),
                VerifierStatus::Passed => ("[x]", Color::Green),
                VerifierStatus::Failed => ("[ ]", Color::Red),
            };
            let status_label = match status {
                VerifierStatus::Pending => "pending",
                VerifierStatus::Running => "running...",
                VerifierStatus::Passed => "passed",
                VerifierStatus::Failed => "FAILED",
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", icon),
                    Style::default().fg(color),
                ),
                Span::styled(
                    name.clone(),
                    Style::default().fg(Color::White),
                ),
                Span::raw("  "),
                Span::styled(
                    status_label,
                    Style::default().fg(color),
                ),
            ]))
        })
        .collect();
    let verifier_list = List::new(verifier_items).block(
        Block::default()
            .title(" Verifiers ")
            .borders(Borders::ALL),
    );
    frame.render_widget(verifier_list, chunks[1]);

    // Logs
    let log_border_color = if app.scroll_focus == ScrollFocus::Log {
        Color::Yellow
    } else {
        Color::White
    };
    let log_items: Vec<ListItem> = app
        .logs
        .iter()
        .map(|l| ListItem::new(format!(" > {}", l)))
        .collect();
    let visible_log_height = chunks[2].height.saturating_sub(2) as usize;
    let log_offset = if app.logs.len() > visible_log_height {
        (app.log_scroll as usize).min(app.logs.len().saturating_sub(visible_log_height))
    } else {
        0
    };
    let visible_logs: Vec<ListItem> = log_items
        .into_iter()
        .skip(log_offset)
        .take(visible_log_height)
        .collect();
    let log_list = List::new(visible_logs).block(
        Block::default()
            .title(" Log ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(log_border_color)),
    );
    frame.render_widget(log_list, chunks[2]);

    // File contents
    let file_border_color = if app.scroll_focus == ScrollFocus::File {
        Color::Yellow
    } else {
        Color::White
    };
    let file_para = Paragraph::new(app.file_contents.as_str())
        .block(
            Block::default()
                .title(format!(
                    " File: {} ",
                    app.file_manager
                        .as_ref()
                        .map(|fm| fm.path.display().to_string())
                        .unwrap_or_default()
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(file_border_color)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.file_scroll, 0));
    frame.render_widget(file_para, chunks[3]);

    // Help bar
    let mut help_spans = vec![
        Span::styled(" q: Quit ", Style::default().fg(Color::Red)),
        Span::raw(" | "),
        Span::styled(" Tab: Switch focus ", Style::default().fg(Color::Cyan)),
        Span::raw(" | "),
        Span::styled(
            " Up/Down: Scroll ",
            Style::default().fg(Color::Cyan),
        ),
    ];
    if app.screen == Screen::Done {
        help_spans.push(Span::raw(" | "));
        help_spans.push(Span::styled(
            " Ctrl+N: New prompt ",
            Style::default().fg(Color::Green),
        ));
    }
    let help = Line::from(help_spans);
    let help_bar = Paragraph::new(help);
    frame.render_widget(help_bar, chunks[4]);
}
