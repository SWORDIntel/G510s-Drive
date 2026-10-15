use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StyledDrawable},
    text::Text,
};

use crate::framebuffer::G510Framebuffer;

pub const TOTAL_PAGES: usize = 5;

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

pub fn render_header(fb: &mut G510Framebuffer, page_idx: usize, title: &str, clock: &str) {
    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    let header_str = format!("[{}/{}:{}]", page_idx + 1, TOTAL_PAGES, title);
    let _ = Text::new(&header_str, Point::new(1, 8), text_style).draw(fb);
    let _ = Text::new(clock, Point::new(128, 8), text_style).draw(fb);
}

/// Page 0: System Telemetry & Thermals
pub fn render_system_screen(
    fb: &mut G510Framebuffer,
    cpu_pct: f32,
    mem_pct: f32,
    temp_c: Option<u32>,
    load1: f32,
    clock: &str,
) {
    fb.clear();
    render_header(fb, 0, "SYS", clock);
    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    // CPU Row (y=21)
    let _ = Text::new("CPU", Point::new(1, 21), text_style).draw(fb);
    let _ = draw_bar(fb, 24, 14, 75, cpu_pct);
    let temp_str = match temp_c {
        Some(t) => format!("{:>3.0}%{:>3}°C", cpu_pct, t),
        None => format!("{:>3.0}%", cpu_pct),
    };
    let _ = Text::new(&temp_str, Point::new(103, 21), text_style).draw(fb);

    // RAM Row (y=37)
    let _ = Text::new("RAM", Point::new(1, 37), text_style).draw(fb);
    let _ = draw_bar(fb, 24, 30, 75, mem_pct);
    let mem_str = format!("{:>3.0}%{:>4.1}L", mem_pct, load1);
    let _ = Text::new(&mem_str, Point::new(103, 37), text_style).draw(fb);
}

/// Page 1: Network & VPN Throughput
pub fn render_network_screen(
    fb: &mut G510Framebuffer,
    iface: &str,
    ip: &str,
    rx_kbs: f32,
    tx_kbs: f32,
    vpn_active: bool,
    clock: &str,
) {
    fb.clear();
    render_header(fb, 1, "NET", clock);
    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    let vpn_str = if vpn_active { "VPN:UP" } else { "VPN:--" };
    let row1 = format!("IF: {:<7} {}", iface, vpn_str);
    let _ = Text::new(&row1, Point::new(1, 19), text_style).draw(fb);

    let row2 = format!("IP: {}", ip);
    let _ = Text::new(&row2, Point::new(1, 30), text_style).draw(fb);

    let row3 = format!("▼{:>5.1}K/s  ▲{:>5.1}K/s", rx_kbs, tx_kbs);
    let _ = Text::new(&row3, Point::new(1, 41), text_style).draw(fb);
}

/// Page 2: Security & Workbench Ticker
pub fn render_security_screen(
    fb: &mut G510Framebuffer,
    failed_ssh: u32,
    vms: u32,
    containers: u32,
    clock: &str,
) {
    fb.clear();
    render_header(fb, 2, "SEC", clock);
    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    let row1 = format!("SSH FAILS (1h): {:>3}", failed_ssh);
    let _ = Text::new(&row1, Point::new(1, 19), text_style).draw(fb);

    let row2 = format!("VMS: {:<2}  DOCKER: {:<3}", vms, containers);
    let _ = Text::new(&row2, Point::new(1, 30), text_style).draw(fb);

    let status = if failed_ssh > 5 { "ALERT: UNUSUAL AUTH" } else { "WORKBENCH SECURE" };
    let _ = Text::new(status, Point::new(1, 41), text_style).draw(fb);
}

/// Page 3: MPRIS Media Player
pub fn render_media_screen(
    fb: &mut G510Framebuffer,
    status: &str,
    artist: &str,
    title: &str,
    clock: &str,
) {
    fb.clear();
    render_header(fb, 3, "MEDIA", clock);
    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    let status_line = format!("STATUS: {}", status);
    let _ = Text::new(&status_line, Point::new(1, 19), text_style).draw(fb);

    let artist_trunc: String = artist.chars().take(25).collect();
    let _ = Text::new(&artist_trunc, Point::new(1, 30), text_style).draw(fb);

    let title_trunc: String = title.chars().take(25).collect();
    let _ = Text::new(&title_trunc, Point::new(1, 41), text_style).draw(fb);
}

/// Page 4: AI Agent & Workbench Status
pub fn render_agent_screen(
    fb: &mut G510Framebuffer,
    state: &str,
    task: &str,
    role: &str,
    clock: &str,
) {
    fb.clear();
    render_header(fb, 4, "AGENT", clock);
    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    let state_line = format!("STATE: {}", state);
    let _ = Text::new(&state_line, Point::new(1, 19), text_style).draw(fb);

    let task_line = format!("TASK: {}", task.chars().take(20).collect::<String>());
    let _ = Text::new(&task_line, Point::new(1, 30), text_style).draw(fb);

    let role_line = format!("ROLE: {}", role.chars().take(20).collect::<String>());
    let _ = Text::new(&role_line, Point::new(1, 41), text_style).draw(fb);
}

/// Notification Toast Overlay
pub fn render_toast_screen(fb: &mut G510Framebuffer, message: &str) {
    fb.clear();
    let stroke_style = PrimitiveStyleBuilder::new()
        .stroke_color(BinaryColor::On)
        .stroke_width(1)
        .build();

    // Border box
    let _ = Rectangle::new(Point::new(2, 2), Size::new(156, 39)).draw_styled(&stroke_style, fb);

    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    for (i, line) in message.lines().take(3).enumerate() {
        let y = 14 + (i as i32) * 12;
        let _ = Text::new(line, Point::new(8, y), text_style).draw(fb);
    }
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
    fn test_carousel_screens_render_pixels() {
        let mut fb = G510Framebuffer::new();

        render_system_screen(&mut fb, 50.0, 75.0, Some(49), 1.5, "12:34");
        assert!(fb.pack_report(false)[32..].iter().any(|&b| b > 0));

        render_network_screen(&mut fb, "vmbr0", "192.168.1.91", 120.0, 45.0, true, "12:34");
        assert!(fb.pack_report(false)[32..].iter().any(|&b| b > 0));

        render_security_screen(&mut fb, 0, 1, 3, "12:34");
        assert!(fb.pack_report(false)[32..].iter().any(|&b| b > 0));

        render_media_screen(&mut fb, "Playing", "Daft Punk", "Get Lucky", "12:34");
        assert!(fb.pack_report(false)[32..].iter().any(|&b| b > 0));

        render_agent_screen(&mut fb, "WORKING", "Carousel Migration", "Architect", "12:34");
        assert!(fb.pack_report(false)[32..].iter().any(|&b| b > 0));

        render_toast_screen(&mut fb, "NOTIFICATION:\nTest Done!");
        assert!(fb.pack_report(false)[32..].iter().any(|&b| b > 0));
    }
}
