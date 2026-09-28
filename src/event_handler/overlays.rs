use crate::app::App;

pub(super) fn in_overlay_mode(app: &App) -> bool {
    app.search_mode
}

pub(super) fn select_searched_service(app: &mut App) {
    let query = app.search_query.to_lowercase();
    if let Some(index) = app
        .services
        .iter()
        .position(|service| service.name.to_lowercase().contains(&query))
    {
        app.state.select(Some(index));
    }
}
