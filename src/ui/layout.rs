use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy)]
pub(super) struct AppAreas {
    pub(super) header: Rect,
    pub(super) body: Rect,
    pub(super) input: Rect,
    pub(super) footer: Rect,
}

pub(super) fn app_areas(area: Rect, desired_input_height: u16) -> AppAreas {
    let input_height = desired_input_height.min(area.height);
    let input_y = area
        .y
        .saturating_add(area.height.saturating_sub(input_height));
    let input = Rect {
        x: area.x,
        y: input_y,
        width: area.width,
        height: input_height,
    };

    let above_input = area.height.saturating_sub(input_height);
    let footer_height = if above_input >= 8 { 2 } else { 0 };
    let footer_y = input_y.saturating_sub(footer_height);
    let footer = Rect {
        x: area.x,
        y: footer_y,
        width: area.width,
        height: footer_height,
    };

    let above_footer = above_input.saturating_sub(footer_height);
    let header_height = above_footer.min(4);
    let header = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: header_height,
    };

    let body_y = area.y.saturating_add(header_height);
    let body_height = above_footer.saturating_sub(header_height);
    let body = Rect {
        x: area.x,
        y: body_y,
        width: area.width,
        height: body_height,
    };

    AppAreas {
        header,
        body,
        input,
        footer,
    }
}
