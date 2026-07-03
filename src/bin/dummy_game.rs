//! Dummy Game - a stand-in "game" window for testing the Overnotes overlay.
//!
//! - Renders a continuously animated scene (scrolling gradient, bouncing
//!   square, frame counter bar) so overlay compositing and transparency are
//!   easy to judge.
//! - Shows received input: clicks paint expanding rings, key presses flash
//!   the border. The window title also logs the last input, which makes
//!   click-through verification unambiguous.
//! - F11 toggles borderless fullscreen. Move/resize/minimize freely to test
//!   overlay tracking.
//!
//! Run with: `cargo run --bin dummy_game`

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;

use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

struct Ring {
    x: f64,
    y: f64,
    born: f64,
}

struct App {
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    start: Instant,
    frame: u64,
    cursor: (f64, f64),
    rings: Vec<Ring>,
    key_flash_until: f64,
    last_input: String,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            surface: None,
            start: Instant::now(),
            frame: 0,
            cursor: (0.0, 0.0),
            rings: Vec::new(),
            key_flash_until: 0.0,
            last_input: String::from("none"),
        }
    }

    fn now(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn update_title(&self) {
        if let Some(window) = &self.window {
            window.set_title(&format!(
                "Dummy Game - Overnotes Test | frame {} | last input: {}",
                self.frame, self.last_input
            ));
        }
    }

    fn draw(&mut self) {
        let t = self.now();
        let frame = self.frame;
        let key_flash_until = self.key_flash_until;
        self.rings.retain(|r| t - r.born < 1.2);

        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else {
            return;
        };
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };
        if surface.resize(w, h).is_err() {
            return;
        }
        let Ok(mut buffer) = surface.buffer_mut() else {
            return;
        };
        let width = size.width as usize;
        let height = size.height as usize;

        // Scrolling diagonal gradient.
        for y in 0..height {
            let fy = y as f64 / height.max(1) as f64;
            for x in 0..width {
                let fx = x as f64 / width.max(1) as f64;
                let wave = ((fx * 4.0 + fy * 3.0 + t * 0.35) * std::f64::consts::TAU * 0.25).sin();
                let r = (28.0 + 38.0 * (0.5 + 0.5 * wave)) as u32;
                let g = (32.0 + 30.0 * (0.5 + 0.5 * (wave + fy).cos())) as u32;
                let b = (66.0 + 80.0 * fy) as u32;
                buffer[y * width + x] = (r << 16) | (g << 8) | b;
            }
        }

        // Bouncing square.
        let sq = 90.0_f64;
        let bx = ((t * 160.0) % (2.0 * (width as f64 - sq).max(1.0))).abs();
        let bx = if bx > (width as f64 - sq) {
            2.0 * (width as f64 - sq) - bx
        } else {
            bx
        };
        let by = ((t * 120.0) % (2.0 * (height as f64 - sq).max(1.0))).abs();
        let by = if by > (height as f64 - sq) {
            2.0 * (height as f64 - sq) - by
        } else {
            by
        };
        fill_rect(
            &mut buffer,
            width,
            height,
            bx as i64,
            by as i64,
            sq as i64,
            sq as i64,
            0x00ff9d4d,
        );

        // Click rings (fade over 1.2 s).
        for ring in &self.rings {
            let age = t - ring.born;
            let radius = 12.0 + age * 90.0;
            let bright = (1.0 - age / 1.2).max(0.0);
            let color = ((255.0 * bright) as u32) << 16
                | ((90.0 * bright) as u32) << 8
                | (90.0 * bright) as u32;
            draw_ring(
                &mut buffer,
                width,
                height,
                ring.x,
                ring.y,
                radius,
                4.0,
                color,
            );
        }

        // Key-press border flash.
        if t < key_flash_until {
            let thickness = 8;
            fill_rect(&mut buffer, width, height, 0, 0, width as i64, thickness, 0x0050ff96);
            fill_rect(
                &mut buffer,
                width,
                height,
                0,
                height as i64 - thickness,
                width as i64,
                thickness,
                0x0050ff96,
            );
            fill_rect(&mut buffer, width, height, 0, 0, thickness, height as i64, 0x0050ff96);
            fill_rect(
                &mut buffer,
                width,
                height,
                width as i64 - thickness,
                0,
                thickness,
                height as i64,
                0x0050ff96,
            );
        }

        // Frame-counter activity bar along the bottom.
        let bar_w = ((frame % 120) as f64 / 120.0 * width as f64) as i64;
        fill_rect(
            &mut buffer,
            width,
            height,
            0,
            height as i64 - 5,
            bar_w,
            5,
            0x00ffffff,
        );

        let _ = buffer.present();
        self.frame += 1;
        if self.frame % 30 == 0 {
            self.update_title();
        }
    }
}

fn fill_rect(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    x: i64,
    y: i64,
    w: i64,
    h: i64,
    color: u32,
) {
    for yy in y.max(0)..(y + h).min(height as i64) {
        for xx in x.max(0)..(x + w).min(width as i64) {
            buffer[yy as usize * width + xx as usize] = color;
        }
    }
}

fn draw_ring(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    cx: f64,
    cy: f64,
    radius: f64,
    thickness: f64,
    color: u32,
) {
    let x0 = ((cx - radius - thickness).floor() as i64).max(0);
    let x1 = ((cx + radius + thickness).ceil() as i64).min(width as i64);
    let y0 = ((cy - radius - thickness).floor() as i64).max(0);
    let y1 = ((cy + radius + thickness).ceil() as i64).min(height as i64);
    for y in y0..y1 {
        for x in x0..x1 {
            let d = ((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)).sqrt();
            if (d - radius).abs() <= thickness {
                buffer[y as usize * width + x as usize] = color;
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Dummy Game - Overnotes Test")
            .with_inner_size(LogicalSize::new(1280.0, 720.0));
        let window = Rc::new(event_loop.create_window(attrs).expect("create window"));
        let context = Context::new(window.clone()).expect("softbuffer context");
        let surface = Surface::new(&context, window.clone()).expect("softbuffer surface");
        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => {
                let now = self.now();
                self.rings.push(Ring {
                    x: self.cursor.0,
                    y: self.cursor.1,
                    born: now,
                });
                self.last_input = format!(
                    "{:?} click @ ({:.0}, {:.0})",
                    button_name(button),
                    self.cursor.0,
                    self.cursor.1
                );
                self.update_title();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if event.logical_key == Key::Named(NamedKey::F11) {
                        if let Some(window) = &self.window {
                            let next = if window.fullscreen().is_some() {
                                None
                            } else {
                                Some(Fullscreen::Borderless(None))
                            };
                            window.set_fullscreen(next);
                        }
                    }
                    self.key_flash_until = self.now() + 0.35;
                    self.last_input = format!("key {:?}", event.logical_key);
                    self.update_title();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn button_name(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "left",
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
        _ => "other",
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run app");
}
