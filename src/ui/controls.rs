use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, Focus};

use super::theme;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let mode = if app.search_mode {
        " FILTER "
    } else if app.focus == Focus::Services {
        " SERVICES "
    } else {
        " ACTIVITY "
    };
    let controls = Paragraph::new(controls_line(app)).block(
        Block::new()
            .borders(Borders::TOP)
            .border_style(Style::new().fg(theme::MUTED))
            .title(Span::styled(
                mode,
                Style::new()
                    .fg(if app.search_mode {
                        theme::TRANSITION
                    } else {
                        theme::FOCUS
                    })
                    .add_modifier(Modifier::BOLD),
            )),
    );

    frame.render_widget(controls, area);
}

fn controls_line(app: &App) -> Line<'static> {
    if app.search_mode {
        return command_line([
            ("Enter".to_string(), "select"),
            ("Esc".to_string(), "cancel"),
            ("type".to_string(), "filter projects"),
        ]);
    }

    let app_keys = &app.keybinds.app;
    let navigation_key = format!("{}/{}", app_keys.scroll_down, app_keys.scroll_up);

    if app.focus == Focus::Services {
        command_line([
            (app.keybinds.services.toggle.clone(), "start/stop"),
            (app.keybinds.services.auto_restart.clone(), "auto-start"),
            (app_keys.search.clone(), "filter"),
            (navigation_key, "move"),
            (app_keys.focus_logs.clone(), "activity"),
            (app_keys.quit.clone(), "quit"),
        ])
    } else {
        command_line([
            (app.keybinds.logs.toggle_auto_scroll.clone(), "auto-scroll"),
            (
                format!("{}/{}", app_keys.switch_tab_left, app_keys.switch_tab_right),
                "view",
            ),
            (navigation_key, "scroll"),
            (app_keys.focus_services.clone(), "services"),
            (app_keys.refresh.clone(), "refresh"),
            (app_keys.quit.clone(), "quit"),
        ])
    }
}

fn command_line<const N: usize>(commands: [(String, &'static str); N]) -> Line<'static> {
    let mut spans = Vec::with_capacity(commands.len() * 4);
    for (index, (key, label)) in commands.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", Style::new().fg(theme::MUTED)));
        }
        let key = key_name(key);
        spans.push(Span::styled(
            format!(" {key} "),
            Style::new()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED),
        ));
        spans.push(Span::styled(
            format!(" {label}"),
            Style::new().fg(theme::TEXT),
        ));
    }
    Line::from(spans)
}

fn key_name(key: String) -> String {
    if key == " " { "Space".to_string() } else { key }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_keeps_key_and_label_order() {
        let line = command_line([("s".to_string(), "start/stop"), ("q".to_string(), "quit")]);
        let text: String = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();

        assert_eq!(text, " s  start/stop   q  quit");
    }

    #[test]
    fn space_binding_has_a_visible_label() {
        assert_eq!(key_name(" ".to_string()), "Space");
    }
}
