use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui::{self, Frame}};
use crate::controls::{Aircraft, ControlMode, FlightMode, MainCamera};

#[derive(Resource)]
pub struct VerticalSpeedTracker {
    pub last_altitude: f32,
    pub last_time: f32,
    pub vertical_speed: f32,
}

impl Default for VerticalSpeedTracker {
    fn default() -> Self {
        Self {
            last_altitude: 0.0,
            last_time: 0.0,
            vertical_speed: 0.0,
        }
    }
}

pub fn flight_hud_system(
    mut contexts: EguiContexts,
    control_mode: Res<ControlMode>,
    aircraft_query: Query<(&Transform, &Aircraft), (With<Aircraft>, Without<MainCamera>)>,
    camera_query: Query<&Transform, With<MainCamera>>,
    time: Res<Time>,
    mut vs_tracker: ResMut<VerticalSpeedTracker>,
) {
    if control_mode.mode != FlightMode::Aircraft {
        return;
    }

    let Ok((plane_transform, aircraft)) = aircraft_query.single() else { return };
    let Ok(_camera_transform) = camera_query.single() else { return };

    let altitude = plane_transform.translation.y;
    let speed = aircraft.speed;
    let throttle = aircraft.throttle * 100.0;
    
    let forward = plane_transform.forward().as_vec3();
    let heading = calculate_heading(forward);
    let pitch = calculate_pitch(forward);
    let roll = calculate_roll(plane_transform);
    
    let current_time = time.elapsed_secs();
    let delta_time = current_time - vs_tracker.last_time;
    
    if delta_time > 0.1 {
        let delta_altitude = altitude - vs_tracker.last_altitude;
        let fpm = (delta_altitude / delta_time) * 60.0;
        vs_tracker.vertical_speed = vs_tracker.vertical_speed * 0.8 + fpm * 0.2;
        vs_tracker.last_altitude = altitude;
        vs_tracker.last_time = current_time;
    }
    
    let vertical_speed = vs_tracker.vertical_speed;

    egui::Window::new("Flight HUD")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-20.0, -20.0])
        .fixed_size([550.0, 300.0])
        .frame(Frame::default().fill(bevy_egui::egui::Color32::from_rgba_unmultiplied(50,50, 50,100)))
        .show(contexts.ctx_mut().unwrap(), |ui| {
            ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);
            ui.spacing_mut().item_spacing = egui::Vec2::new(8.0, 4.0);

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(90.0);
                    ui.label(egui::RichText::new("ALTITUDE").size(12.0));
                    draw_altitude_tape(ui, altitude);
                });

                ui.add_space(10.0);

                ui.vertical(|ui| {
                    ui.set_min_width(160.0);
                    ui.label(egui::RichText::new("ATTITUDE").size(12.0));
                    draw_artificial_horizon(ui, pitch, roll);
                    ui.horizontal(|ui| {
                        ui.label(format!("Pitch: {:.1}°", pitch));
                        ui.label(format!("Roll: {:.1}°", roll));
                    });
                });

                ui.add_space(10.0);

                ui.vertical(|ui| {
                    ui.set_min_width(90.0);
                    ui.label(egui::RichText::new("AIRSPEED").size(12.0));
                    draw_airspeed_tape(ui, speed);
                });
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(100.0);
                    ui.label("HEADING");
                    ui.label(egui::RichText::new(format!("{:.0}°", heading))
                        .size(18.0));
                    draw_compass_rose(ui, heading);
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                ui.vertical(|ui| {
                    ui.set_min_width(60.0);
                    ui.label("THROTTLE");
                    draw_throttle_bar(ui, aircraft.throttle);
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                ui.vertical(|ui| {
                    ui.set_min_width(90.0);
                    ui.label(egui::RichText::new("V/S").size(12.0));
                    draw_vertical_speed_tape(ui, vertical_speed);
                });
            });
        });
}

fn calculate_heading(forward: Vec3) -> f32 {
    let angle = f32::atan2(forward.x, forward.z).to_degrees();
    if angle < 0.0 {
        360.0 + angle
    } else {
        angle
    }
}

fn calculate_pitch(forward: Vec3) -> f32 {
    let horizontal_magnitude = (forward.x * forward.x + forward.z * forward.z).sqrt();
    f32::atan2(forward.y, horizontal_magnitude).to_degrees()
}

fn calculate_roll(transform: &Transform) -> f32 {
    let right = transform.right();
    f32::atan2(right.y, right.x.hypot(right.z)).to_degrees()
}

fn draw_compass_rose(ui: &mut egui::Ui, heading: f32) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        
        let directions = ["N", "NE", "E", "SE", "S", "SW", "W", "NW", "N"];
        let angles = [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0, 360.0];
        
        for i in 0..directions.len() {
            let angle_diff = (heading - angles[i]).abs();
            let angle_diff = if angle_diff > 180.0 { 360.0 - angle_diff } else { angle_diff };
            
            if angle_diff < 45.0 {
                let intensity = 1.0 - (angle_diff / 45.0);
                let color = egui::Color32::from_rgba_premultiplied(
                    (255.0 * intensity) as u8,
                    (255.0 * intensity) as u8,
                    (255.0 * intensity) as u8,
                    (255.0 * intensity) as u8,
                );
                ui.label(egui::RichText::new(directions[i])
                    .color(color)
                    .size(14.0));
            }
        }
    });
}

fn draw_artificial_horizon(ui: &mut egui::Ui, pitch: f32, roll: f32) {
    ui.vertical_centered(|ui| {
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(160.0, 160.0),
            egui::Sense::hover(),
        );
        
        let center = response.rect.center();
        let radius = 75.0;
        
        let sky_color = egui::Color32::from_rgb(50, 120, 200);
        let ground_color = egui::Color32::from_rgb(100, 70, 40);
        
        painter.circle_filled(
            center,
            radius,
            sky_color,
        );
        
        let pitch_offset = -(pitch / 90.0) * radius;
        let roll_rad = roll.to_radians();
        let cos_roll = roll_rad.cos();
        let sin_roll = roll_rad.sin();
        
        let rotate_point = |x: f32, y: f32| -> egui::Pos2 {
            egui::Pos2::new(
                center.x + (x * cos_roll - y * sin_roll),
                center.y + (x * sin_roll + y * cos_roll),
            )
        };
        
        let horizon_left = rotate_point(-radius * 2.0, -pitch_offset);
        let horizon_right = rotate_point(radius * 2.0, -pitch_offset);
        let bottom_left = rotate_point(-radius * 2.0, radius * 2.0);
        let bottom_right = rotate_point(radius * 2.0, radius * 2.0);
        
        painter.add(egui::Shape::convex_polygon(
            vec![horizon_left, horizon_right, bottom_right, bottom_left],
            ground_color,
            egui::Stroke::NONE,
        ));
        
        for i in -6..=6 {
            let angle = i as f32 * 10.0;
            let y_offset = (angle / 90.0) * radius - pitch_offset;
            
            if y_offset.abs() < radius * 1.5 {
                let line_length = if i % 3 == 0 { 40.0 } else { 20.0 };
                let stroke_width = if i % 3 == 0 { 2.0 } else { 1.0 };
                
                painter.line_segment(
                    [
                        rotate_point(-line_length / 2.0, y_offset),
                        rotate_point(line_length / 2.0, y_offset),
                    ],
                    egui::Stroke::new(stroke_width, egui::Color32::WHITE),
                );
                
                if i != 0 && i % 3 == 0 {
                    painter.text(
                        rotate_point(line_length / 2.0 + 15.0, y_offset),
                        egui::Align2::LEFT_CENTER,
                        format!("{}", angle.abs() as i32),
                        egui::FontId::proportional(10.0),
                        egui::Color32::WHITE,
                    );
                }
            }
        }
        
        painter.line_segment(
            [
                egui::Pos2::new(center.x - 60.0, center.y),
                egui::Pos2::new(center.x - 10.0, center.y),
            ],
            egui::Stroke::new(3.0, egui::Color32::YELLOW),
        );
        painter.line_segment(
            [
                egui::Pos2::new(center.x + 10.0, center.y),
                egui::Pos2::new(center.x + 60.0, center.y),
            ],
            egui::Stroke::new(3.0, egui::Color32::YELLOW),
        );
        
        painter.circle_stroke(
            center,
            radius,
            egui::Stroke::new(2.0, egui::Color32::WHITE),
        );
    });
}

fn draw_altitude_tape(ui: &mut egui::Ui, altitude: f32) {
    ui.vertical(|ui| {
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(90.0, 160.0),
            egui::Sense::hover(),
        );
        
        let rect = response.rect;
        let center_y = rect.center().y;
        
        painter.rect_filled(
            rect,
            egui::CornerRadius::same(5),
            egui::Color32::from_rgba_premultiplied(20, 20, 20, 200),
        );
        
        let altitude_step = 25.0;
        let pixels_per_foot = 1.0;
        
        let start_alt = ((altitude - 500.0) / altitude_step).floor() * altitude_step;
        let end_alt = start_alt + 1000.0;
        
        for alt in (start_alt as i32..=end_alt as i32).step_by(altitude_step as usize) {
            let offset = (altitude - alt as f32) * pixels_per_foot;
            let y_pos = center_y + offset;
            
            if y_pos >= rect.top() && y_pos <= rect.bottom() {
                let is_major = alt % 100 == 0;
                let tick_len = if is_major { 25.0 } else { 15.0 };
                
                painter.line_segment(
                    [
                        egui::Pos2::new(rect.right() - tick_len, y_pos),
                        egui::Pos2::new(rect.right(), y_pos),
                    ],
                    egui::Stroke::new(1.5, egui::Color32::WHITE),
                );
                
                if is_major {
                    painter.text(
                        egui::Pos2::new(rect.left() + 5.0, y_pos),
                        egui::Align2::LEFT_CENTER,
                        format!("{}", alt),
                        egui::FontId::proportional(12.0),
                        egui::Color32::WHITE,
                    );
                }
            }
        }
        
        painter.rect_filled(
            egui::Rect::from_center_size(
                egui::Pos2::new(rect.center().x, center_y),
                egui::Vec2::new(80.0, 30.0),
            ),
            egui::CornerRadius::same(3),
            egui::Color32::from_rgb(0, 100, 0),
        );
        
        painter.text(
            egui::Pos2::new(rect.center().x, center_y),
            egui::Align2::CENTER_CENTER,
            format!("{:.0}", altitude),
            egui::FontId::proportional(18.0),
            egui::Color32::WHITE,
        );
    });
}

fn draw_airspeed_tape(ui: &mut egui::Ui, speed: f32) {
    ui.vertical(|ui| {
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(90.0, 160.0),
            egui::Sense::hover(),
        );
        
        let rect = response.rect;
        let center_y = rect.center().y;
        
        painter.rect_filled(
            rect,
            egui::CornerRadius::same(5),
            egui::Color32::from_rgba_premultiplied(20, 20, 20, 200),
        );
        
        let speed_step = 20.0;
        let pixels_per_knot = 2.0;
        
        let start_speed = ((speed - 100.0) / speed_step).floor() * speed_step;
        let end_speed = start_speed + 200.0;
        
        for spd in (start_speed as i32..=end_speed as i32).step_by(speed_step as usize) {
            if spd < 0 { continue; }
            
            let offset = (speed - spd as f32) * pixels_per_knot;
            let y_pos = center_y + offset;
            
            if y_pos >= rect.top() && y_pos <= rect.bottom() {
                let is_major = spd % 50 == 0;
                let tick_len = if is_major { 25.0 } else { 15.0 };
                
                painter.line_segment(
                    [
                        egui::Pos2::new(rect.left(), y_pos),
                        egui::Pos2::new(rect.left() + tick_len, y_pos),
                    ],
                    egui::Stroke::new(1.5, egui::Color32::WHITE),
                );
                
                if is_major {
                    painter.text(
                        egui::Pos2::new(rect.right() - 5.0, y_pos),
                        egui::Align2::RIGHT_CENTER,
                        format!("{}", spd),
                        egui::FontId::proportional(12.0),
                        egui::Color32::WHITE,
                    );
                }
            }
        }
        
        painter.rect_filled(
            egui::Rect::from_center_size(
                egui::Pos2::new(rect.center().x, center_y),
                egui::Vec2::new(80.0, 30.0),
            ),
            egui::CornerRadius::same(3),
            egui::Color32::from_rgb(0, 50, 100),
        );
        
        painter.text(
            egui::Pos2::new(rect.center().x, center_y),
            egui::Align2::CENTER_CENTER,
            format!("{:.0}", speed),
            egui::FontId::proportional(18.0),
            egui::Color32::WHITE,
        );
    });
}

fn draw_throttle_bar(ui: &mut egui::Ui, throttle: f32) {
    ui.vertical_centered(|ui| {
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(50.0, 100.0),
            egui::Sense::hover(),
        );
        
        let rect = response.rect;
        
        painter.rect_filled(
            rect,
            egui::CornerRadius::same(3),
            egui::Color32::from_rgba_premultiplied(30, 30, 30, 200),
        );
        
        let fill_height = rect.height() * throttle;
        let fill_rect = egui::Rect::from_min_max(
            egui::Pos2::new(rect.left(), rect.bottom() - fill_height),
            rect.max,
        );
        
        painter.rect_filled(
            fill_rect,
            egui::CornerRadius::same(3),
            egui::Color32::from_rgb(255, 150, 0),
        );
        
        for i in 0..=10 {
            let y = rect.bottom() - (rect.height() * i as f32 / 10.0);
            let tick_len = if i % 2 == 0 { 8.0 } else { 4.0 };
            
            painter.line_segment(
                [
                    egui::Pos2::new(rect.left(), y),
                    egui::Pos2::new(rect.left() + tick_len, y),
                ],
                egui::Stroke::new(1.0, egui::Color32::WHITE),
            );
        }
        
        painter.text(
            egui::Pos2::new(rect.center().x, rect.center().y),
            egui::Align2::CENTER_CENTER,
            format!("{:.0}%", throttle * 100.0),
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
        
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(3),
            egui::Stroke::new(1.0, egui::Color32::WHITE),
            egui::StrokeKind::Middle,
        );
    });
}

fn draw_vertical_speed_tape(ui: &mut egui::Ui, vertical_speed: f32) {
    ui.vertical(|ui| {
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(90.0, 160.0),
            egui::Sense::hover(),
        );
        
        let rect = response.rect;
        let center_y = rect.center().y;
        
        painter.rect_filled(
            rect,
            egui::CornerRadius::same(5),
            egui::Color32::from_rgba_premultiplied(20, 20, 20, 200),
        );
        
        let vs_step = 100.0;
        let pixels_per_fpm = 0.8;
        
        let start_vs = ((vertical_speed - 500.0) / vs_step).floor() * vs_step;
        let end_vs = start_vs + 1000.0;
        
        for vs in (start_vs as i32..=end_vs as i32).step_by(vs_step as usize) {
            let offset = (vertical_speed - vs as f32) * pixels_per_fpm;
            let y_pos = center_y + offset;
            
            if y_pos >= rect.top() && y_pos <= rect.bottom() {
                let is_major = vs % 500 == 0;
                let tick_len = if is_major { 25.0 } else { 15.0 };
                
                painter.line_segment(
                    [
                        egui::Pos2::new(rect.left(), y_pos),
                        egui::Pos2::new(rect.left() + tick_len, y_pos),
                    ],
                    egui::Stroke::new(1.5, egui::Color32::WHITE),
                );
                
                if is_major {
                    painter.text(
                        egui::Pos2::new(rect.right() - 5.0, y_pos),
                        egui::Align2::RIGHT_CENTER,
                        format!("{}", vs),
                        egui::FontId::proportional(11.0),
                        egui::Color32::WHITE,
                    );
                }
            }
        }
        
        let box_color = if vertical_speed > 50.0 {
            egui::Color32::from_rgb(0, 100, 0)
        } else if vertical_speed < -50.0 {
            egui::Color32::from_rgb(100, 50, 0)
        } else {
            egui::Color32::from_rgb(50, 50, 50)
        };
        
        painter.rect_filled(
            egui::Rect::from_center_size(
                egui::Pos2::new(rect.center().x, center_y),
                egui::Vec2::new(80.0, 30.0),
            ),
            egui::CornerRadius::same(3),
            box_color,
        );
        
        painter.text(
            egui::Pos2::new(rect.center().x, center_y),
            egui::Align2::CENTER_CENTER,
            format!("{:.0}", vertical_speed),
            egui::FontId::proportional(16.0),
            egui::Color32::WHITE,
        );
    });
}
