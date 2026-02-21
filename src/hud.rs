use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui::{self, Frame}};
use crate::controls::{Aircraft, ControlMode, FlightMode, MainCamera, Wind};
use crate::network::{self, NetworkClient, DEFAULT_SERVER_ADDR};
use crossbeam_channel;

pub fn flight_hud_system(
    mut contexts: EguiContexts,
    control_mode: Res<ControlMode>,
    aircraft_query: Query<(&Transform, &Aircraft), (With<Aircraft>, Without<MainCamera>)>,
    camera_query: Query<&Transform, With<MainCamera>>,
    wind: Res<Wind>,
) {
    if control_mode.mode != FlightMode::Aircraft {
        return;
    }

    let Ok((plane_transform, aircraft)) = aircraft_query.single() else { return };
    let Ok(_camera_transform) = camera_query.single() else { return };

    let altitude = plane_transform.translation.y;
    let speed = aircraft.speed;
    
    let forward = plane_transform.forward().as_vec3();
    let heading = calculate_heading(forward);
    let pitch = calculate_pitch(forward);
    let roll = calculate_roll(plane_transform);
    
    let ctx = contexts.ctx_mut().unwrap();
    let window_frame = Frame::default().fill(bevy_egui::egui::Color32::from_rgba_unmultiplied(50, 50, 50, 100));
    
    // Display crash warning
    if aircraft.crashed {
        egui::Window::new("Crash")
            .title_bar(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([300.0, 100.0])
            .frame(Frame::default().fill(egui::Color32::from_rgba_unmultiplied(200, 0, 0, 200)))
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(egui::RichText::new("⚠ AIRCRAFT CRASHED ⚠").size(24.0).strong());
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("Press R to respawn").size(14.0));
                });
            });
    }
    
    egui::Window::new("Attitude")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-20.0, -20.0])
        .fixed_size([180.0, 220.0])
        .frame(window_frame)
        .show(ctx, |ui| {
            ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("ATTITUDE").size(12.0));
                draw_artificial_horizon(ui, pitch, roll);
                ui.horizontal(|ui| {
                    ui.label(format!("Pitch: {:.1}°", pitch));
                    ui.label(format!("Roll: {:.1}°", roll));
                });
            });
        });
    
    egui::Window::new("Altitude")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-210.0, -20.0])
        .fixed_size([110.0, 200.0])
        .frame(window_frame)
        .show(ctx, |ui| {
            ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("ALTITUDE").size(12.0));
                draw_altitude_tape(ui, altitude);
                ui.horizontal(|ui| {
                    ui.label(format!("{}", altitude));
                });
            });
        });
    
    egui::Window::new("Throttle")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::LEFT_BOTTOM, [150.0, -20.0])
        .fixed_size([120.0, 70.0])
        .frame(window_frame)
        .show(ctx, |ui| {
            ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("THROTTLE").size(12.0));
                draw_throttle_gauge(ui, aircraft.throttle, aircraft.max_throttle, aircraft.speed, aircraft.max_speed);
                ui.horizontal(|ui| {
                    ui.label(format!("0{:?}", aircraft.throttle));
                });
            });
        });
    
    egui::Window::new("Heading")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 20.0])
        .fixed_size([150.0, 150.0])
        .frame(window_frame)
        .show(ctx, |ui| {
            ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);
            ui.vertical_centered(|ui| {
                ui.label("HEADING & WIND");
                
                let wind_heading = calculate_wind_heading(&wind);
                draw_wind_compass(ui, heading, wind_heading, wind.wind_speed);
                
                ui.label(egui::RichText::new(format!("HDG: {:.0}°", heading))
                    .size(11.0));
                ui.label(egui::RichText::new(format!("Wind: {:.0}° @ {:.1}", wind_heading, wind.wind_speed))
                    .size(10.0));
            });
        });
    
    egui::Window::new("Airspeed")
        .title_bar(false)
        .resizable(false)
        .fixed_size([110.0, 200.0])
        .anchor(egui::Align2::LEFT_BOTTOM, [20.0, -20.0])
        .frame(window_frame)
        .show(ctx, |ui| {
            ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("AIRSPEED").size(12.0));
                draw_airspeed_tape(ui, speed, aircraft);
                ui.horizontal(|ui| {
                    ui.label(format!("{}", aircraft.speed));
                });
            });
        });
}

fn calculate_heading(forward: Vec3) -> f32 {
    let angle = f32::atan2(forward.x, -forward.z).to_degrees() + 90.0;
    if angle < 0.0 {
        360.0 + angle
    } else if angle >= 360.0 {
        angle - 360.0
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

fn calculate_wind_heading(wind: &Wind) -> f32 {
    let dir = wind.wind_direction;
    let angle = f32::atan2(dir.x, -dir.z).to_degrees() + 90.0;
    if angle < 0.0 {
        360.0 + angle
    } else if angle >= 360.0 {
        angle - 360.0
    } else {
        angle
    }
}

fn draw_wind_compass(ui: &mut egui::Ui, aircraft_heading: f32, wind_heading: f32, wind_speed: f32) {
    let (response, painter) = ui.allocate_painter(
        egui::Vec2::new(90.0, 90.0),
        egui::Sense::hover(),
    );
    
    let rect = response.rect;
    let center = rect.center();
    let radius = 30.0;
    
    painter.circle_stroke(
        center,
        radius,
        egui::Stroke::new(1.5, egui::Color32::WHITE),
    );
    
    let directions = ["N", "E", "S", "W"];
    let angles = [0.0, 90.0, 180.0, 270.0];
    
    for i in 0..directions.len() {
        let rotated_angle = angles[i] - aircraft_heading;
        let angle_rad = (rotated_angle - 90.0).to_radians();
        let pos = egui::Pos2::new(
            center.x + (radius + 10.0) * angle_rad.cos(),
            center.y + (radius + 10.0) * angle_rad.sin(),
        );
        
        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            directions[i],
            egui::FontId::proportional(15.0),
            egui::Color32::WHITE,
        );
    }
    
    for deg in (0..360).step_by(30) {
        let rotated_angle = deg as f32 - aircraft_heading;
        let angle_rad = (rotated_angle - 90.0).to_radians();
        let is_cardinal = deg % 90 == 0;
        let tick_start = if is_cardinal { radius - 8.0 } else { radius - 4.0 };
        
        let p1 = egui::Pos2::new(
            center.x + tick_start * angle_rad.cos(),
            center.y + tick_start * angle_rad.sin(),
        );
        let p2 = egui::Pos2::new(
            center.x + radius * angle_rad.cos(),
            center.y + radius * angle_rad.sin(),
        );
        
        painter.line_segment(
            [p1, p2],
            egui::Stroke::new(1.0, egui::Color32::GRAY),
        );
    }
    
    painter.line_segment(
        [
            egui::Pos2::new(center.x, center.y - radius - 7.0),
            egui::Pos2::new(center.x, center.y - radius + 7.0),
        ],
        egui::Stroke::new(3.0, egui::Color32::WHITE),
    );
    
    let relative_wind_heading = wind_heading - aircraft_heading;
    let wind_angle_rad = (relative_wind_heading - 90.0).to_radians();
    let arrow_length = radius * 0.85;
    
    let tip = egui::Pos2::new(
        center.x + arrow_length * wind_angle_rad.cos(),
        center.y + arrow_length * wind_angle_rad.sin(),
    );
    
    let wind_intensity = (wind_speed / 50.0).min(1.0);
    let wind_color = egui::Color32::from_rgb(
        (100.0 + 155.0 * wind_intensity) as u8,
        (200.0 - 100.0 * wind_intensity) as u8,
        255,
    );
    
    painter.line_segment(
        [center, tip],
        egui::Stroke::new(2.5, wind_color),
    );
    
    let arrow_head_size = 8.0;
    let arrow_head_angle = 25.0_f32.to_radians();
    
    let left_angle = wind_angle_rad + std::f32::consts::PI - arrow_head_angle;
    let right_angle = wind_angle_rad + std::f32::consts::PI + arrow_head_angle;
    
    let left_point = egui::Pos2::new(
        tip.x + arrow_head_size * left_angle.cos(),
        tip.y + arrow_head_size * left_angle.sin(),
    );
    let right_point = egui::Pos2::new(
        tip.x + arrow_head_size * right_angle.cos(),
        tip.y + arrow_head_size * right_angle.sin(),
    );
    
    painter.line_segment([tip, left_point], egui::Stroke::new(2.5, wind_color));
    painter.line_segment([tip, right_point], egui::Stroke::new(2.5, wind_color));
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
        
        let arrow_width = 80.0;
        let arrow_height = 30.0;
        let arrow_tip_width = 10.0;
        let center_x = rect.center().x;
        
        let arrow_points = vec![
            egui::Pos2::new(center_x - arrow_width / 2.0, center_y - arrow_height / 2.0),
            egui::Pos2::new(center_x + arrow_width / 2.0 - arrow_tip_width, center_y - arrow_height / 2.0),
            egui::Pos2::new(center_x + arrow_width / 2.0, center_y),
            egui::Pos2::new(center_x + arrow_width / 2.0 - arrow_tip_width, center_y + arrow_height / 2.0),
            egui::Pos2::new(center_x - arrow_width / 2.0, center_y + arrow_height / 2.0),
        ];
        
        painter.add(egui::Shape::convex_polygon(
            arrow_points,
            egui::Color32::from_rgb(40, 40, 40),
            egui::Stroke::new(1.0, egui::Color32::WHITE),
        ));
        
        painter.text(
            egui::Pos2::new(center_x - 5.0, center_y),
            egui::Align2::CENTER_CENTER,
            format!("{:.0}", altitude),
            egui::FontId::proportional(18.0),
            egui::Color32::WHITE,
        );
    });
}

fn draw_airspeed_tape(ui: &mut egui::Ui, speed: f32, aircraft: &Aircraft) {
    ui.vertical(|ui| {
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(90.0, 160.0),
            egui::Sense::hover(),
        );
        
        let rect = response.rect;
        let center_y = rect.center().y;

        let aircraft_max_speed = aircraft.max_speed;
        
        let speed_ranges = [
            (0.0,                      aircraft_max_speed * 0.3, egui::Color32::from_rgb(150, 0, 0)),
            (aircraft_max_speed * 0.3, aircraft_max_speed * 0.5, egui::Color32::from_rgb(200, 200, 0)),
            (aircraft_max_speed * 0.5, aircraft_max_speed * 1.0, egui::Color32::from_rgb(0, 150, 0)),
            (aircraft_max_speed * 1.0, aircraft_max_speed * 1.2, egui::Color32::from_rgb(200, 200, 0)),
            (aircraft_max_speed * 1.2, aircraft_max_speed * 2.0, egui::Color32::from_rgb(150, 0, 0)),
        ];
        
        let speed_step = 10.0;
        let pixels_per_knot = 1.5;
        
        let start_speed = ((speed - 100.0) / speed_step).floor() * speed_step;
        let end_speed = start_speed + 200.0;
        
        for (range_start, range_end, range_color) in speed_ranges.iter() {
            let y_start = center_y + (speed - range_end) * pixels_per_knot;
            let y_end = center_y + (speed - range_start) * pixels_per_knot;
            
            if y_end > rect.top() && y_start < rect.bottom() {
                let y_start_clamped = y_start.max(rect.top());
                let y_end_clamped = y_end.min(rect.bottom());
                
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::Pos2::new(rect.left(), y_start_clamped),
                        egui::Pos2::new(rect.left() + 10.0, y_end_clamped),
                    ),
                    egui::CornerRadius::ZERO,
                    *range_color,
                );
            }
        }
        
        for spd in (start_speed as i32..=end_speed as i32).step_by(speed_step as usize) {
            if spd < 0 { continue; }
            
            let offset = (speed - spd as f32) * pixels_per_knot;
            let y_pos = center_y + offset;
            
            if y_pos >= rect.top() && y_pos <= rect.bottom() {
                let is_major = spd % 50 == 0;
                let tick_len = if is_major { 25.0 } else { 15.0 };
                
                painter.line_segment(
                    [
                        egui::Pos2::new(rect.left() + 5.0, y_pos),
                        egui::Pos2::new(rect.left() + 5.0 + tick_len, y_pos),
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
        
        let arrow_width = 80.0;
        let arrow_height = 30.0;
        let arrow_tip_width = 10.0;
        let center_x = rect.center().x;
        
        let arrow_points = vec![
            egui::Pos2::new(center_x - arrow_width / 2.0 + arrow_tip_width, center_y - arrow_height / 2.0),
            egui::Pos2::new(center_x + arrow_width / 2.0, center_y - arrow_height / 2.0),
            egui::Pos2::new(center_x + arrow_width / 2.0, center_y + arrow_height / 2.0),
            egui::Pos2::new(center_x - arrow_width / 2.0 + arrow_tip_width, center_y + arrow_height / 2.0),
            egui::Pos2::new(center_x - arrow_width / 2.0, center_y),
        ];
        
        painter.add(egui::Shape::convex_polygon(
            arrow_points,
            egui::Color32::from_rgb(40, 40, 40),
            egui::Stroke::new(1.0, egui::Color32::WHITE),
        ));
        
        painter.text(
            egui::Pos2::new(center_x + 5.0, center_y),
            egui::Align2::CENTER_CENTER,
            format!("{:.0}", speed),
            egui::FontId::proportional(18.0),
            egui::Color32::WHITE,
        );
    });
}

fn draw_throttle_gauge(ui: &mut egui::Ui, throttle: f32, max_throttle: f32, speed: f32, max_speed: f32) {
    ui.vertical_centered(|ui| {
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(140.0, 90.0),
            egui::Sense::hover(),
        );
        
        let rect = response.rect;
        let radius = 60.0;
        let center = egui::Pos2::new(rect.center().x, rect.top() + radius + 5.0);
        
        let start_angle = std::f32::consts::PI;
        let end_angle = 0.0;
        let angle_range = start_angle - end_angle;
        
        let num_segments = 100;
        for i in 0..num_segments {
            let t1 = i as f32 / num_segments as f32;
            let t2 = (i + 1) as f32 / num_segments as f32;
            
            let angle1 = start_angle - angle_range * t1;
            let angle2 = start_angle - angle_range * t2;
            
            let color = if t1 > (1.0 / max_throttle) {
                egui::Color32::from_rgb(60, 20, 20)
            } else {
                egui::Color32::from_rgb(40, 40, 40)
            };
            
            let inner_radius = radius - 8.0;
            let p1_outer = egui::Pos2::new(
                center.x + radius * angle1.cos(),
                center.y - radius * angle1.sin(),
            );
            let p2_outer = egui::Pos2::new(
                center.x + radius * angle2.cos(),
                center.y - radius * angle2.sin(),
            );
            let p1_inner = egui::Pos2::new(
                center.x + inner_radius * angle1.cos(),
                center.y - inner_radius * angle1.sin(),
            );
            let p2_inner = egui::Pos2::new(
                center.x + inner_radius * angle2.cos(),
                center.y - inner_radius * angle2.sin(),
            );
            
            painter.add(egui::Shape::convex_polygon(
                vec![p1_outer, p2_outer, p2_inner, p1_inner],
                color,
                egui::Stroke::NONE,
            ));
        }
        
        let num_ticks = (max_throttle * 10.0) as i32;
        for i in 0..=num_ticks {
            let t = i as f32 / num_ticks as f32;
            let angle = start_angle - angle_range * t;
            let throttle_percent = t * max_throttle * 100.0;
            
            let is_major = i % 2 == 0;
            let tick_start = if is_major { radius - 12.0 } else { radius - 8.0 };
            let tick_end = radius;
            
            let tick_color = if throttle_percent > 100.0 {
                egui::Color32::from_rgb(255, 100, 100)
            } else {
                egui::Color32::WHITE
            };
            
            let p1 = egui::Pos2::new(
                center.x + tick_start * angle.cos(),
                center.y - tick_start * angle.sin(),
            );
            let p2 = egui::Pos2::new(
                center.x + tick_end * angle.cos(),
                center.y - tick_end * angle.sin(),
            );
            
            painter.line_segment(
                [p1, p2],
                egui::Stroke::new(1.5, tick_color),
            );
            
            if is_major {
                let label_radius = radius - 20.0;
                let label_pos = egui::Pos2::new(
                    center.x + label_radius * angle.cos(),
                    center.y - label_radius * angle.sin(),
                );
                
                painter.text(
                    label_pos,
                    egui::Align2::CENTER_CENTER,
                    format!("{:.0}", throttle_percent),
                    egui::FontId::proportional(10.0),
                    tick_color,
                );
            }
        }
        
        let redline_t = 1.0 / max_throttle;
        let redline_angle = start_angle - angle_range * redline_t;
        let redline_start = radius - 15.0;
        let redline_end = radius + 3.0;
        
        let p1 = egui::Pos2::new(
            center.x + redline_start * redline_angle.cos(),
            center.y - redline_start * redline_angle.sin(),
        );
        let p2 = egui::Pos2::new(
            center.x + redline_end * redline_angle.cos(),
            center.y - redline_end * redline_angle.sin(),
        );
        
        painter.line_segment(
            [p1, p2],
            egui::Stroke::new(3.0, egui::Color32::from_rgb(255, 0, 0)),
        );
        
        let throttle_clamped = throttle.min(max_throttle);
        let throttle_angle = start_angle - angle_range * (throttle_clamped / max_throttle);
        
        let needle_color = if throttle > 1.0 {
            egui::Color32::from_rgb(255, 100, 100)
        } else {
            egui::Color32::from_rgb(200, 255, 200)
        };
        
        let needle_length = radius - 15.0;
        let needle_tip = egui::Pos2::new(
            center.x + needle_length * throttle_angle.cos(),
            center.y - needle_length * throttle_angle.sin(),
        );
        
        painter.line_segment(
            [center, needle_tip],
            egui::Stroke::new(3.0, needle_color),
        );
        
        painter.circle_filled(center, 5.0, needle_color);
        
        let speed_ratio = (speed / max_speed).min(max_throttle);
        let speed_angle = start_angle - angle_range * (speed_ratio / max_throttle);
        
        let speed_needle_length = (radius - 15.0) * 0.75;
        let speed_needle_tip = egui::Pos2::new(
            center.x + speed_needle_length * speed_angle.cos(),
            center.y - speed_needle_length * speed_angle.sin(),
        );
        
        let speed_needle_color = egui::Color32::WHITE;
        
        painter.line_segment(
            [center, speed_needle_tip],
            egui::Stroke::new(2.0, speed_needle_color),
        );
        
        painter.circle_filled(center, 3.0, speed_needle_color);
        
        painter.text(
            egui::Pos2::new(center.x, center.y + 10.0),
            egui::Align2::CENTER_TOP,
            format!("{:.0}%", throttle * 100.0),
            egui::FontId::proportional(16.0),
            egui::Color32::WHITE,
        );
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Basic,
    Advanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsPreset {
    Low,
    High,
}

#[derive(Resource)]
pub struct MultiplayerMenu {
    pub server_address: String,
    pub connection_status: String,
    pub connecting: bool,
    pub connection_receiver: Option<crossbeam_channel::Receiver<Result<NetworkClient, String>>>,
    pub settings_tab: SettingsTab,
    pub graphics_preset: GraphicsPreset,
}

impl Default for MultiplayerMenu {
    fn default() -> Self {
        Self {
            server_address: DEFAULT_SERVER_ADDR.to_string(),
            connection_status: String::new(),
            connecting: false,
            connection_receiver: None,
            settings_tab: SettingsTab::Basic,
            graphics_preset: GraphicsPreset::Low,
        }
    }
}

pub fn auto_connect_on_startup(
    mut menu: ResMut<MultiplayerMenu>,
) {
    let address = DEFAULT_SERVER_ADDR.to_string();
    menu.connecting = true;
    menu.connection_status = "Auto-connecting...".to_string();
    
    let (tx, rx) = crossbeam_channel::unbounded();
    menu.connection_receiver = Some(rx);
    
    let value = address.clone();
    std::thread::spawn(move || {
        let result = network::TOKIO_RUNTIME.block_on(network::connect_to_server(&value));
        let _ = tx.send(result);
    });
    
    println!("🌐 Attempting auto-connect to {}", address);
}

pub fn process_connection_results(
    mut menu: ResMut<MultiplayerMenu>,
    mut commands: Commands,
) {
    if let Some(rx) = &menu.connection_receiver {
        if let Ok(result) = rx.try_recv() {
            menu.connecting = false;
            menu.connection_receiver = None;
            
            match result {
                Ok(client) => {
                    menu.connection_status = "Connected!".to_string();
                    commands.insert_resource(client);
                }
                Err(e) => {
                    menu.connection_status = format!("Connection failed: {}", e);
                }
            }
        }
    }
}
