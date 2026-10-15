use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StyledDrawable},
    text::Text,
};

use crate::framebuffer::G510Framebuffer;

pub fn draw_bar(
    fb: &mut G510Framebuffer,
    x: i32,
    y: i32,
    width: u32,
    pct: f32,
) -> Result<(), core::convert::Infallible> {
    let stroke_style = PrimitiveStyleBuilder::new()
        .stroke_color(BinaryColor::On)
        .stroke_width(1)
        .build();

    // Outline rectangle (height = 8)
    Rectangle::new(Point::new(x, y), Size::new(width, 8)).draw_styled(&stroke_style, fb)?;

    let clamped_pct = pct.clamp(0.0, 100.0);
    let inner_width = (width.saturating_sub(4) as f32 * (clamped_pct / 100.0)).round() as u32;

    if inner_width > 0 {
        let fill_style = PrimitiveStyle::with_fill(BinaryColor::On);
        Rectangle::new(Point::new(x + 2, y + 2), Size::new(inner_width, 4))
            .draw_styled(&fill_style, fb)?;
    }

    Ok(())
}

pub fn render_status_screen(
    fb: &mut G510Framebuffer,
    cpu_pct: f32,
    mem_pct: f32,
    clock: &str,
) {
    fb.clear();
    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    // Header: "G510 / SYSTEM" and clock
    let _ = Text::new("G510 / SYSTEM", Point::new(1, 8), text_style).draw(fb);
    let _ = Text::new(clock, Point::new(128, 8), text_style).draw(fb);

    // CPU Row
    let _ = Text::new("CPU", Point::new(1, 22), text_style).draw(fb);
    let _ = draw_bar(fb, 26, 15, 95, cpu_pct);
    let cpu_str = format!("{:>3.0}%", cpu_pct);
    let _ = Text::new(&cpu_str, Point::new(126, 22), text_style).draw(fb);

    // RAM Row
    let _ = Text::new("RAM", Point::new(1, 37), text_style).draw(fb);
    let _ = draw_bar(fb, 26, 30, 95, mem_pct);
    let mem_str = format!("{:>3.0}%", mem_pct);
    let _ = Text::new(&mem_str, Point::new(126, 37), text_style).draw(fb);
}

pub fn render_text_screen(fb: &mut G510Framebuffer, message: &str) {
    fb.clear();
    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    for (i, line) in message.lines().take(3).enumerate() {
        let y = 10 + (i as i32) * 14;
        let _ = Text::new(line, Point::new(2, y), text_style).draw(fb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_screen_rendering() {
        let mut fb = G510Framebuffer::new();
        render_text_screen(&mut fb, "HELLO\nWORLD");
        let report = fb.pack_report(false);
        // There should be active pixels in the report payload (after byte 32)
        let active_bytes: usize = report[32..].iter().map(|b| b.count_ones() as usize).sum();
        assert!(active_bytes > 0);
    }

    #[test]
    fn test_status_screen_rendering() {
        let mut fb = G510Framebuffer::new();
        render_status_screen(&mut fb, 50.0, 75.0, "12:34");
        let report = fb.pack_report(false);
        let active_bytes: usize = report[32..].iter().map(|b| b.count_ones() as usize).sum();
        assert!(active_bytes > 50);
    }
}
