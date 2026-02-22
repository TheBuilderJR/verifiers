use std::collections::VecDeque;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::{App, Screen, ScrollFocus, SetupFocus, VerifierStatus};

/// Compute visual row widths produced by word-wrapping a single line (no newlines),
/// matching ratatui's WordWrapper with trim=false.
fn word_wrap_widths(line: &str, max_width: u16) -> Vec<u16> {
    if max_width == 0 {
        return vec![0];
    }

    let mut wrapped: Vec<u16> = Vec::new();
    let mut line_width: u16 = 0;
    let mut word_width: u16 = 0;
    let mut whitespace_width: u16 = 0;
    let mut non_ws_prev = false;
    let mut pending_ws: VecDeque<u16> = VecDeque::new();
    let mut has_pending_word = false;

    for ch in line.chars() {
        let is_ws = ch.is_whitespace();
        let sym_w = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;

        if sym_w > max_width {
            continue;
        }

        let word_found = non_ws_prev && is_ws;
        let untrimmed_overflow =
            line_width == 0 && (word_width + whitespace_width + sym_w > max_width);

        // Commit pending whitespace + word to line
        if word_found || untrimmed_overflow {
            line_width += whitespace_width + word_width;
            whitespace_width = 0;
            word_width = 0;
            pending_ws.clear();
            has_pending_word = false;
        }

        let line_full = line_width >= max_width;
        let pending_overflow =
            sym_w > 0 && line_width + whitespace_width + word_width >= max_width;

        if line_full || pending_overflow {
            let mut remaining = max_width.saturating_sub(line_width);
            wrapped.push(line_width);
            line_width = 0;

            // Remove whitespace that fits in the remaining space of the pushed line
            while let Some(&w) = pending_ws.front() {
                if w > remaining {
                    break;
                }
                whitespace_width -= w;
                remaining -= w;
                pending_ws.pop_front();
            }

            // Skip first whitespace after a line break
            if is_ws && pending_ws.is_empty() {
                non_ws_prev = false;
                continue;
            }
        }

        if is_ws {
            whitespace_width += sym_w;
            pending_ws.push_back(sym_w);
        } else {
            word_width += sym_w;
            has_pending_word = true;
        }

        non_ws_prev = !is_ws;
    }

    // Finalization (matches ratatui's process_input)
    if line_width == 0 && !has_pending_word && !pending_ws.is_empty() {
        wrapped.push(0);
    }
    let final_width = line_width + whitespace_width + word_width;
    if final_width > 0 || has_pending_word {
        wrapped.push(final_width);
    }
    if wrapped.is_empty() {
        wrapped.push(0);
    }

    wrapped
}

/// Compute (x_offset, y_offset) cursor position at the end of text rendered with
/// ratatui's Wrap { trim: false } word wrapping, matching the visual layout exactly.
fn cursor_pos_wrapped(text: &str, max_width: u16) -> (u16, u16) {
    if max_width == 0 {
        return (0, 0);
    }

    let mut total_rows: u16 = 0;
    let natural_lines: Vec<&str> = text.split('\n').collect();

    for (i, natural_line) in natural_lines.iter().enumerate() {
        let widths = word_wrap_widths(natural_line, max_width);
        if i < natural_lines.len() - 1 {
            total_rows += widths.len() as u16;
        } else {
            total_rows += widths.len().saturating_sub(1) as u16;
            let last_width = widths.last().copied().unwrap_or(0);
            return (last_width, total_rows);
        }
    }

    (0, total_rows)
}

/// Count total visual rows after word-wrapping text with trim=false.
fn wrapped_row_count(text: &str, max_width: u16) -> u16 {
    text.split('\n')
        .map(|line| word_wrap_widths(line, max_width).len() as u16)
        .sum()
}

pub fn draw(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Setup => draw_setup(frame, app),
        Screen::Running | Screen::Done => draw_running(frame, app),
    }
}

fn draw_setup(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Calculate dynamic heights for verifier input fields based on word wrapping
    let inner_width = area.width.saturating_sub(2); // subtract borders
    let name_rows = wrapped_row_count(&app.verifier_name_input, inner_width);
    let vprompt_rows = wrapped_row_count(&app.verifier_prompt_input, inner_width);

    // Build help spans early so we can calculate dynamic height
    let can_start = app.can_start();
    let start_hint = if can_start {
        Span::styled(" Ctrl+S: Start ", Style::default().fg(Color::Green))
    } else {
        Span::styled(
            " Ctrl+S: Start (need prompt + enabled verifier) ",
            Style::default().fg(Color::DarkGray),
        )
    };
    let mut help_spans = vec![
        Span::styled(" Tab: Next field ", Style::default().fg(Color::Cyan)),
        Span::raw(" | "),
    ];
    if app.setup_focus == SetupFocus::VerifierList {
        help_spans.push(Span::styled(
            " Up/Down: Select ",
            Style::default().fg(Color::Cyan),
        ));
        help_spans.push(Span::raw(" | "));
        help_spans.push(Span::styled(
            " Space: Toggle ",
            Style::default().fg(Color::Cyan),
        ));
        help_spans.push(Span::raw(" | "));
        help_spans.push(Span::styled(
            " Ctrl+D: Remove ",
            Style::default().fg(Color::Cyan),
        ));
        help_spans.push(Span::raw(" | "));
    } else {
        help_spans.push(Span::styled(
            " Enter: Add verifier ",
            Style::default().fg(Color::Cyan),
        ));
        help_spans.push(Span::raw(" | "));
    }
    help_spans.push(start_hint);
    if !app.prompt_history.is_empty() && app.setup_focus == SetupFocus::Prompt {
        help_spans.push(Span::raw(" | "));
        help_spans.push(Span::styled(
            " Ctrl+P/N: History ",
            Style::default().fg(Color::Cyan),
        ));
    }
    help_spans.push(Span::raw(" | "));
    help_spans.push(Span::styled(" Ctrl+C/q: Quit ", Style::default().fg(Color::Red)));

    // Calculate help bar height: text rows + 1 for top border
    let help_text_width: usize = help_spans.iter().map(|s| s.content.width()).sum();
    let help_bar_rows = if area.width > 0 {
        ((help_text_width.max(1) + area.width as usize - 1) / area.width as usize) as u16
    } else {
        1
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),                // Title
            Constraint::Min(6),                  // Prompt input
            Constraint::Length(name_rows + 2),    // Verifier name input (dynamic)
            Constraint::Length(vprompt_rows + 2), // Verifier prompt input (dynamic)
            Constraint::Min(4),                  // Verifier list
            Constraint::Length(help_bar_rows + 1), // Help bar (dynamic + top border)
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
    let name_text = Paragraph::new(app.verifier_name_input.as_str())
        .block(name_block)
        .wrap(Wrap { trim: false });
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
    let vprompt_text = Paragraph::new(app.verifier_prompt_input.as_str())
        .block(vprompt_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(vprompt_text, chunks[3]);

    // Verifier list
    let list_focused = app.setup_focus == SetupFocus::VerifierList;
    let list_border_style = if list_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };
    let enabled_count = app.verifiers.iter().filter(|v| v.enabled).count();
    let verifier_items: Vec<ListItem> = app
        .verifiers
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let checkbox = if v.enabled { "[x]" } else { "[ ]" };
            let text = format!("  {} {}. {} — {}", checkbox, i + 1, v.name, v.prompt);
            if list_focused && i == app.selected_verifier {
                ListItem::new(text).style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
            } else if !v.enabled {
                ListItem::new(text).style(Style::default().fg(Color::DarkGray))
            } else {
                ListItem::new(text)
            }
        })
        .collect();
    let verifier_list = List::new(verifier_items).block(
        Block::default()
            .title(format!(" Verifiers ({}/{} enabled) ", enabled_count, app.verifiers.len()))
            .borders(Borders::ALL)
            .border_style(list_border_style),
    );
    frame.render_widget(verifier_list, chunks[4]);

    // Render help bar
    let help = Line::from(help_spans);
    let help_bar = Paragraph::new(help)
        .block(Block::default().borders(Borders::TOP))
        .wrap(Wrap { trim: false });
    frame.render_widget(help_bar, chunks[5]);

    // Show cursor in the focused input, using word-wrap-aware positioning
    match app.setup_focus {
        SetupFocus::Prompt => {
            let iw = chunks[1].width.saturating_sub(2);
            if iw > 0 {
                let (x_off, y_off) = cursor_pos_wrapped(&app.prompt_input, iw);
                let x = chunks[1].x + 1 + x_off;
                let y = chunks[1].y + 1 + y_off;
                frame.set_cursor_position((x, y));
            }
        }
        SetupFocus::VerifierName => {
            let iw = chunks[2].width.saturating_sub(2);
            if iw > 0 {
                let (x_off, y_off) = cursor_pos_wrapped(&app.verifier_name_input, iw);
                let x = chunks[2].x + 1 + x_off;
                let y = chunks[2].y + 1 + y_off;
                frame.set_cursor_position((x, y));
            }
        }
        SetupFocus::VerifierPrompt => {
            let iw = chunks[3].width.saturating_sub(2);
            if iw > 0 {
                let (x_off, y_off) = cursor_pos_wrapped(&app.verifier_prompt_input, iw);
                let x = chunks[3].x + 1 + x_off;
                let y = chunks[3].y + 1 + y_off;
                frame.set_cursor_position((x, y));
            }
        }
        SetupFocus::VerifierList => {
            // No text cursor in the list view
        }
    }
}

fn draw_running(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Build help spans early so we can calculate dynamic height
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
            " Ctrl+E: Edit prompt ",
            Style::default().fg(Color::Green),
        ));
        help_spans.push(Span::raw(" | "));
        help_spans.push(Span::styled(
            " Ctrl+N: New prompt ",
            Style::default().fg(Color::Green),
        ));
    }

    let help_text_width: usize = help_spans.iter().map(|s| s.content.width()).sum();
    let help_bar_rows = if area.width > 0 {
        ((help_text_width.max(1) + area.width as usize - 1) / area.width as usize) as u16
    } else {
        1
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),                   // Title + status
            Constraint::Length(app.verifier_statuses.len() as u16 + 2), // Verifier checklist
            Constraint::Percentage(40),              // Logs
            Constraint::Percentage(40),              // File contents
            Constraint::Length(help_bar_rows),        // Help bar (dynamic)
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

    // Render help bar
    let help = Line::from(help_spans);
    let help_bar = Paragraph::new(help).wrap(Wrap { trim: false });
    frame.render_widget(help_bar, chunks[4]);
}
