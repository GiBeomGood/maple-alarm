use eframe::egui;
use egui::{Color32, Frame, Pos2, Rect, RichText, Sense, Vec2, ViewportCommand};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use crate::state::SharedState;
use crate::alarm;

const WINDOW_W: f32 = 180.0;
const BG: Color32 = Color32::from_rgb(26, 26, 26);
const TEXT: Color32 = Color32::from_rgb(220, 220, 220);
const BTN_NORMAL: Color32 = Color32::from_rgb(45, 45, 45);
const DOT_NORMAL: Color32 = Color32::from_rgb(68, 255, 136);
const DOT_ALARM: Color32 = Color32::from_rgb(255, 68, 68);
const BTN_FLASH: Color32 = Color32::from_rgb(240, 120, 120);

pub struct AlarmApp {
    shared_state: Arc<SharedState>,
    tray: tray_icon::TrayIcon,
    exit_id: tray_icon::menu::MenuId,
    confirm_flash_until: Option<Instant>,
}

impl AlarmApp {
    pub fn new(
        cc: &eframe::CreationContext,
        state: Arc<SharedState>,
        tray: tray_icon::TrayIcon,
        exit_id: tray_icon::menu::MenuId,
    ) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/malgun.ttf") {
            fonts.font_data.insert(
                "malgun".to_owned(),
                egui::FontData::from_owned(bytes).into(),
            );
            fonts.families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "malgun".to_owned());
        }
        cc.egui_ctx.set_fonts(fonts);

        let mut visuals = egui::Visuals::dark();
        visuals.window_shadow = egui::Shadow::NONE;
        cc.egui_ctx.set_visuals(visuals);

        Self {
            shared_state: state,
            tray,
            exit_id,
            confirm_flash_until: None,
        }
    }
}

impl eframe::App for AlarmApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        use tray_icon::menu::MenuEvent;
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.exit_id {
                std::process::exit(0);
            }
        }

        let state = &self.shared_state;
        let remaining = state.remaining_secs.load(Ordering::Acquire);
        let is_alarm = remaining == 0;

        let now = Instant::now();
        let is_flash = self.confirm_flash_until.map(|t| t > now).unwrap_or(false);
        if !is_flash {
            self.confirm_flash_until = None;
        }

        let time_text = if is_alarm {
            "00:00".to_string()
        } else {
            format!("{}:{:02}", remaining / 60, remaining % 60)
        };

        self.tray.set_tooltip(Some(&format!("AlarmApp - {}", time_text))).ok();

        let btn_color = if is_flash {
            BTN_FLASH
        } else if is_alarm {
            let t = ctx.input(|i| i.time);
            let phase = ((t * 2.5).sin() as f32 + 1.0) * 0.5;
            Color32::from_rgb(
                (155.0 + phase * 75.0) as u8,
                (25.0 + phase * 25.0) as u8,
                (25.0 + phase * 25.0) as u8,
            )
        } else {
            BTN_NORMAL
        };

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(BG))
            .show(ctx, |ui| {
                let rect = ui.max_rect();

                // Drag zone (excludes button area at y=68+)
                let drag_rect = Rect::from_min_size(rect.min, Vec2::new(WINDOW_W, 68.0));
                let drag = ui.interact(drag_rect, egui::Id::new("drag"), Sense::drag());
                if drag.dragged() {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }

                // Status dot
                let dot_pos = Pos2::new(rect.min.x + 20.0, rect.min.y + 19.0);
                ui.painter().circle_filled(
                    dot_pos,
                    4.0,
                    if is_alarm { DOT_ALARM } else { DOT_NORMAL },
                );

                // Caption
                let caption = if is_alarm { "알람" } else { "다음 알람까지" };
                ui.put(
                    Rect::from_min_size(
                        Pos2::new(rect.min.x + 32.0, rect.min.y + 10.0),
                        Vec2::new(140.0, 18.0),
                    ),
                    egui::Label::new(RichText::new(caption).size(12.0).color(TEXT)),
                );

                // Time
                ui.put(
                    Rect::from_min_size(
                        Pos2::new(rect.min.x + 12.0, rect.min.y + 33.0),
                        Vec2::new(156.0, 28.0),
                    ),
                    egui::Label::new(RichText::new(&time_text).size(20.0).color(TEXT)),
                );

                // Confirm button
                let btn_rect = Rect::from_min_size(
                    Pos2::new(rect.min.x + 12.0, rect.min.y + 68.0),
                    Vec2::new(156.0, 20.0),
                );
                let resp = ui.put(
                    btn_rect,
                    egui::Button::new(RichText::new("확인").size(12.0).color(Color32::WHITE))
                        .fill(btn_color)
                        .stroke(egui::Stroke::NONE)
                        .min_size(Vec2::new(156.0, 20.0)),
                );

                if resp.clicked() && is_alarm && !is_flash {
                    state.alarm_active.store(false, Ordering::Release);
                    state.remaining_secs.store(state.reset_secs, Ordering::Release);
                    alarm::play_confirm_sound();
                    self.confirm_flash_until =
                        Some(Instant::now() + Duration::from_millis(150));
                }
            });

        if is_alarm || is_flash {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.102, 0.102, 0.102, 1.0]
    }
}
