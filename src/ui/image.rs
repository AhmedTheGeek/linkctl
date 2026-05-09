use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};
use ratatui::Frame;

use super::panel_block;
use crate::app::App;
use crate::camera::{CtrlId, CtrlKind};

pub const IMAGE_ROWS: &[CtrlId] = &[
    CtrlId::Brightness,
    CtrlId::Contrast,
    CtrlId::Saturation,
    CtrlId::Hue,
    CtrlId::Sharpness,
    CtrlId::WbAuto,
    CtrlId::WbTemp,
    CtrlId::FocusAuto,
    CtrlId::Focus,
    CtrlId::PowerLineFreq,
];

pub fn draw(f: &mut Frame, area: Rect, app: &App, focused: bool) {
    let block = panel_block("Image", focused);
    let inner = block.inner(area);
    f.render_widget(block.clone(), area);

    let items: Vec<ListItem> = IMAGE_ROWS
        .iter()
        .map(|id| ListItem::new(format_row(*id, app, inner.width)))
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::new()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    if focused {
        state.select(Some(app.image_cursor.min(IMAGE_ROWS.len().saturating_sub(1))));
    }
    f.render_stateful_widget(list, inner, &mut state);
}

fn format_row(id: CtrlId, app: &App, width: u16) -> Line<'static> {
    let label = id.label();
    let desc = match app.camera.desc(id) {
        Some(d) => d,
        None => {
            return Line::from(vec![
                Span::raw(format!("  {label:<14}")),
                Span::from("(unsupported)").dim(),
            ]);
        }
    };
    let val = app.value(id).unwrap_or(desc.default);
    let inactive = desc.is_inactive();
    let dim = if inactive {
        Style::new().fg(Color::DarkGray)
    } else {
        Style::default()
    };

    match &desc.kind {
        CtrlKind::Bool => {
            let on = val != 0;
            let toggle = if on { "[x]" } else { "[ ]" };
            Line::from(vec![
                Span::styled(format!("  {label:<14}"), dim),
                Span::styled(format!("{toggle} {}", if on { "on" } else { "off" }), dim),
            ])
        }
        CtrlKind::Menu(items) => {
            let pretty = items
                .iter()
                .find(|(v, _)| *v == val)
                .map(|(_, n)| n.clone())
                .unwrap_or_else(|| val.to_string());
            Line::from(vec![
                Span::styled(format!("  {label:<14}"), dim),
                Span::styled(pretty, dim),
            ])
        }
        CtrlKind::Int => {
            // Render a small bar; reserve ~12 cells for label, ~6 for value, rest for bar.
            let total = width as usize;
            let bar_cells = total.saturating_sub(28).max(8);
            let bar = render_bar(val, desc.min, desc.max, bar_cells);
            let extra = if inactive { " (inactive)" } else { "" };
            Line::from(vec![
                Span::styled(format!("  {label:<14}"), dim),
                Span::styled(bar, dim),
                Span::styled(format!(" {val:>6}{extra}"), dim),
            ])
        }
    }
}

fn render_bar(val: i64, min: i64, max: i64, cells: usize) -> String {
    if max <= min || cells == 0 {
        return String::new();
    }
    let span = (max - min) as f64;
    let pos = ((val - min) as f64 / span).clamp(0.0, 1.0);
    let filled = (pos * cells as f64).round() as usize;
    let mut s = String::with_capacity(cells + 2);
    s.push('[');
    for i in 0..cells {
        s.push(if i < filled { '#' } else { '-' });
    }
    s.push(']');
    s
}
