use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

pub(super) fn colorize_logs(logs: &str) -> Text<'static> {
    let mut lines = Vec::with_capacity(logs.lines().count());

    for raw_line in logs.lines() {
        if raw_line.starts_with("Pull output:") {
            lines.push(Line::from(vec![Span::styled(
                "Pull output:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]));
        } else if raw_line.starts_with("Up output:") {
            lines.push(Line::from(vec![Span::styled(
                "Up output:",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )]));
        } else if raw_line.starts_with("Down output:") {
            lines.push(Line::from(vec![Span::styled(
                "Down output:",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )]));
        } else if raw_line.contains("failed")
            || raw_line.contains("Failed")
            || raw_line.contains("error")
            || raw_line.contains("Error")
        {
            lines.push(Line::from(vec![Span::styled(
                raw_line.to_owned(),
                Style::default().fg(Color::Red),
            )]));
        } else if raw_line.contains("success")
            || raw_line.contains("Success")
            || raw_line.contains("done")
            || raw_line.contains("Done")
        {
            lines.push(Line::from(vec![Span::styled(
                raw_line.to_owned(),
                Style::default().fg(Color::Green),
            )]));
        } else if raw_line.trim().is_empty() {
            lines.push(Line::from(""));
        } else {
            lines.push(colorize_runtime_log_line(raw_line));
        }
    }

    Text::from(lines)
}

fn colorize_runtime_log_line(line: &str) -> Line<'static> {
    if let Some((service, body)) = split_service_prefix(line) {
        if let Some((head, marker, tail)) = split_log_marker(body) {
            let marker_level = marker.trim_matches(':').trim().to_ascii_uppercase();
            let mut tail_color = classify_log_body_color(tail);
            if matches!(marker_level.as_str(), "LOG" | "INFO" | "NOTICE" | "*")
                && tail_color == Color::Green
            {
                tail_color = Color::Gray;
            }

            return Line::from(vec![
                Span::styled(
                    format!("{} | ", service),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(head.to_string(), Style::default().fg(Color::DarkGray)),
                Span::styled(" ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    marker.to_string(),
                    Style::default().fg(log_marker_color(marker)),
                ),
                Span::styled(" ", Style::default().fg(Color::DarkGray)),
                Span::styled(tail.to_string(), Style::default().fg(tail_color)),
            ]);
        }

        let body_color = classify_log_body_color(body);

        return Line::from(vec![
            Span::styled(
                format!("{} | ", service),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(body.to_string(), Style::default().fg(body_color)),
        ]);
    }

    Line::from(vec![Span::styled(
        line.to_string(),
        Style::default().fg(classify_log_body_color(line)),
    )])
}

fn split_service_prefix(line: &str) -> Option<(&str, &str)> {
    let (service, body) = line.split_once('|')?;
    if service.trim().is_empty() || body.trim().is_empty() {
        return None;
    }
    Some((service.trim(), body.trim_start()))
}

fn classify_log_body_color(body: &str) -> Color {
    let lower = body.to_ascii_lowercase();

    if lower.contains(" panic")
        || lower.contains(" panicked")
        || lower.contains(" fatal")
        || lower.contains("error")
        || lower.contains("exception")
        || lower.contains("crit")
    {
        return Color::Red;
    }

    if lower.contains("warn")
        || lower.contains("timeout")
        || lower.contains("retry")
        || lower.contains("deprecated")
    {
        return Color::Yellow;
    }

    if lower.contains("debug") || lower.contains("trace") {
        return Color::LightBlue;
    }

    if lower.contains("started")
        || lower.contains("ready")
        || lower.contains("listening")
        || lower.contains("connected")
        || lower.contains("accept connections")
        || lower.contains("created")
        || lower.contains("loaded")
        || lower.contains("ok")
        || lower.contains("success")
    {
        return Color::Green;
    }

    Color::Gray
}

fn split_log_marker(body: &str) -> Option<(&str, &str, &str)> {
    for marker in [
        " TRACE ",
        " DEBUG ",
        " INFO ",
        " NOTICE ",
        " WARN ",
        " WARNING ",
        " ERROR ",
        " ERR ",
        " CRITICAL ",
        " FATAL ",
        " LOG:",
        " WARNING:",
        " ERROR:",
        " FATAL:",
        " * ",
        " # ",
        " - ",
    ] {
        if let Some(index) = body.find(marker) {
            let head = body[..index].trim_end();
            let tail = body[index + marker.len()..].trim_start();
            let symbol = marker.trim();
            if !head.is_empty() && !tail.is_empty() {
                return Some((head, symbol, tail));
            }
        }
    }
    None
}

fn log_marker_color(marker: &str) -> Color {
    match marker
        .trim_matches(':')
        .trim()
        .to_ascii_uppercase()
        .as_str()
    {
        "ERROR" | "ERR" | "FATAL" | "CRITICAL" | "PANIC" => Color::Red,
        "WARN" | "WARNING" | "#" => Color::Yellow,
        "DEBUG" | "TRACE" | "-" => Color::LightBlue,
        "INFO" | "NOTICE" | "LOG" | "*" => Color::Green,
        _ => Color::Gray,
    }
}

pub(super) fn colorize_events(logs: &str) -> Text<'static> {
    let mut lines = Vec::with_capacity(logs.lines().count());

    for raw_line in logs.lines() {
        if raw_line.trim().is_empty() {
            lines.push(Line::from(""));
            continue;
        }

        if let Some(rest) = raw_line.strip_prefix("[event] ") {
            if let Some(runtime_payload) = rest.strip_prefix("runtime ") {
                lines.push(colorize_runtime_event("service", runtime_payload));
                continue;
            }

            if let Some((scope, details)) = rest.split_once(" runtime ") {
                lines.push(colorize_runtime_event(scope, details));
                continue;
            }

            if let Some((scope, action)) = rest.split_once(' ') {
                lines.push(Line::from(vec![
                    Span::styled("[event] ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{} ", scope),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        action.to_string(),
                        Style::default().fg(event_action_color(action)),
                    ),
                ]));
            } else {
                lines.push(Line::from(vec![Span::styled(
                    raw_line.to_string(),
                    Style::default().fg(Color::Gray),
                )]));
            }
        } else {
            lines.push(Line::from(vec![Span::styled(
                raw_line.to_string(),
                Style::default().fg(Color::Gray),
            )]));
        }
    }

    Text::from(lines)
}

fn colorize_runtime_event(scope: &str, details: &str) -> Line<'static> {
    let mut spans = vec![
        Span::styled("[event] ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} ", scope),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("runtime ", Style::default().fg(Color::Yellow)),
    ];

    if let Some(ips_body) = extract_bracket_body(details, "ips=") {
        spans.push(Span::styled("ip ", Style::default().fg(Color::Blue)));
        spans.extend(colorize_compact_ip_mappings(&ips_body));
    }

    if let Some(ports_body) = extract_bracket_body(details, "ports=") {
        if !spans.is_empty() {
            spans.push(Span::styled(" ", Style::default().fg(Color::DarkGray)));
        }
        spans.push(Span::styled("ports ", Style::default().fg(Color::Magenta)));
        spans.extend(colorize_compact_port_mappings(&ports_body));
    } else {
        spans.push(Span::styled(
            details.to_string(),
            Style::default().fg(Color::Gray),
        ));
    }

    Line::from(spans)
}

fn event_action_color(action: &str) -> Color {
    match action {
        "start"
        | "running (snapshot)"
        | "health_status: healthy"
        | "pull cached"
        | "pull complete"
        | "running confirmed"
        | "stopped confirmed" => Color::Green,
        "create" | "restart" | "unpause" | "start requested" | "stop requested"
        | "pulling images" | "up requested" => Color::Yellow,
        "stop" | "destroy" | "pause" | "die" => Color::Red,
        "kill" | "health_status: unhealthy" => Color::LightRed,
        "error" => Color::LightRed,
        action if action.contains("failed") => Color::LightRed,
        action if action.contains("not running") => Color::LightRed,
        _ => Color::Gray,
    }
}

fn extract_bracket_body(details: &str, key: &str) -> Option<String> {
    let start = details.find(key)? + key.len();
    let rest = &details[start..];
    if !rest.starts_with('[') {
        return None;
    }

    let end = rest.find(']')?;
    Some(rest[1..end].to_string())
}

fn colorize_compact_ip_mappings(mappings: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (idx, mapping) in mappings.split(',').map(str::trim).enumerate() {
        if mapping.is_empty() {
            continue;
        }

        if idx > 0 {
            spans.push(Span::styled(", ", Style::default().fg(Color::DarkGray)));
        }

        if let Some((network, ip)) = mapping.split_once('=') {
            spans.push(Span::styled(
                compact_ip_label(network, ip),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                mapping.to_string(),
                Style::default().fg(Color::Gray),
            ));
        }
    }
    spans
}

fn colorize_compact_port_mappings(mappings: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (idx, mapping) in mappings.split(',').map(str::trim).enumerate() {
        if mapping.is_empty() {
            continue;
        }

        if idx > 0 {
            spans.push(Span::styled(", ", Style::default().fg(Color::DarkGray)));
        }

        if let Some((container_port, host_binding)) = mapping.split_once('=') {
            let label = compact_port_label(container_port, host_binding);
            spans.push(Span::styled(
                label,
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                mapping.to_string(),
                Style::default().fg(Color::Gray),
            ));
        }
    }
    spans
}

fn compact_ip_label(network: &str, ip: &str) -> String {
    if ip == "pending" || ip == "unknown" {
        format!("{}:{}", network, ip)
    } else {
        ip.to_string()
    }
}

fn compact_port_label(container_port: &str, host_binding: &str) -> String {
    let container_port = container_port
        .split_once('/')
        .map(|(port, _)| port)
        .unwrap_or(container_port);

    if host_binding == "internal" {
        return format!("{} int", container_port);
    }

    if let Some((_, host_port)) = host_binding.rsplit_once(':') {
        if host_port == container_port {
            host_port.to_string()
        } else {
            format!("{}->{}", container_port, host_port)
        }
    } else {
        format!("{}->{}", container_port, host_binding)
    }
}

#[cfg(test)]
mod tests {
    use super::compact_port_label;

    #[test]
    fn compact_port_label_removes_protocol_and_matching_host_ip() {
        assert_eq!(compact_port_label("1025/tcp", "0.0.0.0:1025"), "1025");
    }

    #[test]
    fn compact_port_label_shows_host_mapping_when_ports_differ() {
        assert_eq!(compact_port_label("8080/tcp", "0.0.0.0:5433"), "8080->5433");
    }

    #[test]
    fn compact_port_label_marks_internal_ports() {
        assert_eq!(compact_port_label("1110/tcp", "internal"), "1110 int");
    }
}
