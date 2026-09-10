use ratatui::{Frame, layout::Rect};

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App) {
    let Some(toast) = &app.toast else {
        return;
    };

    let frame_area = frame.area();
    let width = frame_area.width.saturating_sub(2).min(50);
    let area = Rect {
        x: frame_area
            .x
            .saturating_add(frame_area.width.saturating_sub(width).saturating_sub(1)),
        y: frame_area.y.saturating_add(1),
        width,
        height: frame_area.height.min(3),
    };

    if area.width > 0 && area.height > 0 {
        frame.render_widget(
            crate::toast::create_toast_widget(toast, app.animation_tick),
            area,
        );
    }
}
