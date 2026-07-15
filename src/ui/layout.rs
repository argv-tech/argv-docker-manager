use ratatui::layout::{Constraint, Layout, Margin, Rect};

pub struct Sections {
    pub status_bar: Rect,
    pub services_list: Rect,
    pub logs: Rect,
    pub search: Option<Rect>,
    pub separator: Option<Rect>,
    pub help: Rect,
}

pub fn build(area: Rect, show_search: bool) -> Sections {
    let outer = if area.width >= 72 && area.height >= 20 {
        area.inner(Margin::new(1, 1))
    } else {
        area
    };

    let [status_bar, content, controls] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .areas(outer);

    let (services, separator, logs) = content_areas(content);

    let (search, services_list) = if show_search {
        let [search, services_list] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(services);
        (Some(search), services_list)
    } else {
        (None, services)
    };

    Sections {
        status_bar,
        services_list,
        logs,
        search,
        separator,
        help: controls,
    }
}

fn content_areas(area: Rect) -> (Rect, Option<Rect>, Rect) {
    if area.width >= 96 {
        let service_width = (area.width / 3).clamp(30, 42);
        let [services, separator, logs] = Layout::horizontal([
            Constraint::Length(service_width),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(area);
        (services, Some(separator), logs)
    } else if area.height < 30 {
        let [services, _, logs] = Layout::vertical([
            Constraint::Percentage(42),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(area);
        (services, None, logs)
    } else {
        let [services, _, logs] = Layout::vertical([
            Constraint::Length(12),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(area);
        (services, None, logs)
    }
}
