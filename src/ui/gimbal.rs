use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Points, Rectangle};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::{arcsec_to_deg, panel_block};
use crate::app::App;
use crate::camera::CtrlId;

pub fn draw(f: &mut Frame, area: Rect, app: &App, focused: bool) {
    let block = panel_block("Gimbal", focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).split(inner);

    let pan_min = app
        .bounds
        .get(&CtrlId::Pan)
        .map(|b| b.0 as f64)
        .unwrap_or(-522000.0);
    let pan_max = app
        .bounds
        .get(&CtrlId::Pan)
        .map(|b| b.1 as f64)
        .unwrap_or(522000.0);
    let tilt_min = app
        .bounds
        .get(&CtrlId::Tilt)
        .map(|b| b.0 as f64)
        .unwrap_or(-324000.0);
    let tilt_max = app
        .bounds
        .get(&CtrlId::Tilt)
        .map(|b| b.1 as f64)
        .unwrap_or(360000.0);

    let pan = app.value(CtrlId::Pan).unwrap_or(0) as f64;
    let tilt = app.value(CtrlId::Tilt).unwrap_or(0) as f64;

    let canvas = Canvas::default()
        .marker(Marker::Braille)
        .x_bounds([pan_min, pan_max])
        .y_bounds([tilt_min, tilt_max])
        .paint(move |ctx| {
            // Outline rectangle covering the full pan/tilt envelope.
            ctx.draw(&Rectangle {
                x: pan_min,
                y: tilt_min,
                width: pan_max - pan_min,
                height: tilt_max - tilt_min,
                color: Color::DarkGray,
            });
            // Crosshair at origin.
            ctx.draw(&Points {
                coords: &[(0.0, 0.0)],
                color: Color::DarkGray,
            });
            // Current position dot.
            ctx.draw(&Points {
                coords: &[(pan, tilt)],
                color: Color::Cyan,
            });
        });
    f.render_widget(canvas, rows[0]);

    let pan_deg = arcsec_to_deg(pan as i64);
    let tilt_deg = arcsec_to_deg(tilt as i64);
    let zoom = app.value(CtrlId::Zoom).unwrap_or(100) as f64 / 100.0;

    let info = vec![
        Line::from(vec![
            Span::from("pan  ").dim(),
            Span::styled(
                format!("{pan_deg:+6.1}°"),
                Style::new().fg(Color::Cyan),
            ),
            Span::from(format!(
                "   range {:+.0}°..{:+.0}°",
                arcsec_to_deg(pan_min as i64),
                arcsec_to_deg(pan_max as i64)
            ))
            .dim(),
        ]),
        Line::from(vec![
            Span::from("tilt ").dim(),
            Span::styled(
                format!("{tilt_deg:+6.1}°"),
                Style::new().fg(Color::Cyan),
            ),
            Span::from(format!(
                "   range {:+.0}°..{:+.0}°",
                arcsec_to_deg(tilt_min as i64),
                arcsec_to_deg(tilt_max as i64)
            ))
            .dim(),
        ]),
        Line::from(vec![
            Span::from("zoom ").dim(),
            Span::styled(format!("{zoom:.2}x"), Style::new().fg(Color::Cyan)),
        ]),
    ];
    f.render_widget(Paragraph::new(info), rows[1]);
}
