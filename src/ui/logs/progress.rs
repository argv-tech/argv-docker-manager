use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

use crate::status::Status;

pub(super) fn placeholder_text(message: &'static str) -> (Text<'static>, u16) {
    (
        Text::from(vec![Line::from(vec![Span::styled(
            message,
            Style::default().fg(Color::DarkGray),
        )])]),
        1,
    )
}

pub(super) fn event_progress_line(
    status: &Status,
    pull_progress: Option<&str>,
    tick: u64,
) -> Option<Line<'static>> {
    match status {
        Status::Pulling => {
            let progress = pull_progress.unwrap_or("in progress");
            let bar = progress_bar(tick, parse_progress_percent(progress));
            Some(Line::from(vec![
                Span::styled("[progress] ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}", status),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" [", Style::default().fg(Color::DarkGray)),
                Span::styled(bar, Style::default().fg(Color::Cyan)),
                Span::styled("] ", Style::default().fg(Color::DarkGray)),
                Span::styled(progress.to_string(), Style::default().fg(Color::White)),
                Span::styled(" ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    pulling_spinner(tick).to_string(),
                    Style::default().fg(Color::Cyan),
                ),
            ]))
        }
        Status::Starting => {
            let bar = progress_bar(tick, None);
            Some(Line::from(vec![
                Span::styled("[progress] ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "starting".to_string(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" [", Style::default().fg(Color::DarkGray)),
                Span::styled(bar, Style::default().fg(Color::Yellow)),
                Span::styled("] ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    transition_spinner(tick).to_string(),
                    Style::default().fg(Color::Yellow),
                ),
            ]))
        }
        Status::Stopping => {
            let bar = progress_bar(tick, None);
            Some(Line::from(vec![
                Span::styled("[progress] ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "stopping".to_string(),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" [", Style::default().fg(Color::DarkGray)),
                Span::styled(bar, Style::default().fg(Color::Red)),
                Span::styled("] ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    transition_spinner(tick).to_string(),
                    Style::default().fg(Color::Red),
                ),
            ]))
        }
        _ => None,
    }
}

fn pulling_spinner(tick: u64) -> char {
    const FRAMES: [char; 10] = ['.', ':', 'o', 'O', '@', '*', '@', 'O', 'o', ':'];
    FRAMES[(tick as usize) % FRAMES.len()]
}

fn transition_spinner(tick: u64) -> char {
    const FRAMES: [char; 4] = ['|', '/', '-', '\\'];
    FRAMES[(tick as usize) % FRAMES.len()]
}

fn parse_progress_percent(progress: &str) -> Option<u8> {
    let digits_reversed: String = progress
        .chars()
        .rev()
        .skip_while(|ch| *ch != '%')
        .skip(1)
        .take_while(|ch| ch.is_ascii_digit())
        .collect();

    if digits_reversed.is_empty() {
        return None;
    }

    let digits: String = digits_reversed.chars().rev().collect();
    digits.parse::<u8>().ok().map(|percent| percent.min(100))
}

fn progress_bar(tick: u64, percent: Option<u8>) -> String {
    const WIDTH: usize = 24;
    const MARKER_WIDTH: usize = 5;

    if let Some(percent) = percent {
        let filled = (percent as usize * WIDTH) / 100;
        return format!("{}{}", "#".repeat(filled), "-".repeat(WIDTH - filled));
    }

    let mut bar = vec!['-'; WIDTH];
    let offset = (tick as usize) % (WIDTH + MARKER_WIDTH);
    for i in 0..MARKER_WIDTH {
        let idx = offset + i;
        if idx < WIDTH {
            bar[idx] = '#';
        }
    }

    bar.into_iter().collect()
}
