//! D00M: a from-scratch Doom-style first person shooter.
//!
//! Rendering is a software raycaster (no GPU, no wgpu); all art is generated at
//! start-up in `textures.rs`. The only dependencies are `winit` for the window
//! and `softbuffer` to present the framebuffer.

mod font;
mod game;
mod gfx;
mod map;
mod math;
mod render;
mod shot;
mod textures;

use std::collections::HashSet;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use game::{Game, Input};
use render::{H, Renderer, W};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

const WINDOW_SCALE: f64 = 2.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "D00M\n\n  cargo run --release            play\n  cargo run --release -- --shot frame.bmp [--at X,Y,ANGLE] [--steps N] [--fire] [--map] [--use]\n\n\
             Controls:\n  WASD move / strafe, mouse or arrow keys turn, Shift run\n  Left mouse / Space fire, E use, 1-2 weapons\n  Tab map, R restart, Esc quit"
        );
        return Ok(());
    }

    if let Some(i) = args.iter().position(|a| a == "--shot") {
        let path = args
            .get(i + 1)
            .cloned()
            .unwrap_or_else(|| "frame.bmp".into());
        let mut opts = shot::ShotOptions {
            at: None,
            angle: None,
            steps: 20,
            fire: false,
            show_map: false,
            use_key: false,
        };
        if let Some(j) = args.iter().position(|a| a == "--at")
            && let Some(spec) = args.get(j + 1)
        {
            let parts: Vec<f32> = spec
                .split(',')
                .filter_map(|p| p.trim().parse().ok())
                .collect();
            if parts.len() >= 2 {
                opts.at = Some(math::Vec2::new(parts[0], parts[1]));
                opts.angle = parts.get(2).copied();
            }
        }
        if let Some(n) = args
            .iter()
            .position(|a| a == "--steps")
            .and_then(|j| args.get(j + 1))
            .and_then(|n| n.parse().ok())
        {
            opts.steps = n;
        }
        opts.fire = args.iter().any(|a| a == "--fire");
        opts.show_map = args.iter().any(|a| a == "--map");
        opts.use_key = args.iter().any(|a| a == "--use");
        shot::capture(&path, opts)?;
        println!("wrote {path} ({W}x{H})");
        return Ok(());
    }

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    window: Option<Arc<Window>>,
    context: Option<softbuffer::Context<Arc<Window>>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    renderer: Renderer,
    game: Game,
    keys: HashSet<KeyCode>,
    input: Input,
    mouse_fire: bool,
    grabbed: bool,
    last_frame: Instant,
    last_render: Instant,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            context: None,
            surface: None,
            renderer: Renderer::new(),
            game: Game::new(),
            keys: HashSet::new(),
            input: Input::default(),
            mouse_fire: false,
            grabbed: false,
            last_frame: Instant::now(),
            last_render: Instant::now(),
        }
    }

    fn grab_pointer(window: &Window) {
        if window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
            .is_ok()
        {
            window.set_cursor_visible(false);
        }
    }

    fn apply_held_keys(&mut self) {
        let k = &self.keys;
        self.input.forward = k.contains(&KeyCode::KeyW) || k.contains(&KeyCode::ArrowUp);
        self.input.back = k.contains(&KeyCode::KeyS) || k.contains(&KeyCode::ArrowDown);
        self.input.strafe_l = k.contains(&KeyCode::KeyA);
        self.input.strafe_r = k.contains(&KeyCode::KeyD);
        self.input.turn_l = k.contains(&KeyCode::ArrowLeft);
        self.input.turn_r = k.contains(&KeyCode::ArrowRight);
        self.input.run = k.contains(&KeyCode::ShiftLeft) || k.contains(&KeyCode::ShiftRight);
        self.input.fire =
            self.mouse_fire || k.contains(&KeyCode::Space) || k.contains(&KeyCode::ControlLeft);
    }

    fn clear_one_shots(&mut self) {
        self.input.use_pressed = false;
        self.input.weapon1 = false;
        self.input.weapon2 = false;
        self.input.restart = false;
        self.input.toggle_map = false;
        self.input.mouse_dx = 0.0;
    }

    fn redraw(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }

        // Cap at 60 fps even if the compositor does not vsync.
        let now = Instant::now();
        let elapsed = now - self.last_frame;
        if elapsed < Duration::from_micros(16_600) {
            std::thread::sleep(Duration::from_micros(16_600) - elapsed);
        }
        let dt = self.last_render.elapsed().as_secs_f32().min(0.05);
        self.last_frame = Instant::now();
        self.last_render = self.last_frame;

        self.apply_held_keys();
        self.game.update(dt, &self.input);
        self.clear_one_shots();
        self.renderer.render(&self.game);

        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return;
        };
        if surface.resize(w, h).is_err() {
            return;
        }
        let Ok(mut buffer) = surface.buffer_mut() else {
            return;
        };
        self.renderer
            .blit_to(&mut buffer, size.width as usize, size.height as usize);
        let _ = buffer.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("D00M")
            .with_inner_size(LogicalSize::new(
                W as f64 * WINDOW_SCALE,
                H as f64 * WINDOW_SCALE,
            ))
            .with_min_inner_size(LogicalSize::new(480.0, 300.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("could not open a window: {e}");
                event_loop.exit();
                return;
            }
        };
        let context = match softbuffer::Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("softbuffer context failed: {e}");
                event_loop.exit();
                return;
            }
        };
        let surface = match softbuffer::Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("softbuffer surface failed: {e}");
                event_loop.exit();
                return;
            }
        };
        Self::grab_pointer(&window);
        self.grabbed = true;
        self.window = Some(window.clone());
        self.context = Some(context);
        self.surface = Some(surface);
        self.last_render = Instant::now();
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                match event.state {
                    ElementState::Pressed => {
                        if self.keys.insert(code) && !event.repeat {
                            match code {
                                KeyCode::Escape => event_loop.exit(),
                                KeyCode::KeyE => self.input.use_pressed = true,
                                KeyCode::Tab => self.input.toggle_map = true,
                                KeyCode::KeyR => self.input.restart = true,
                                KeyCode::Digit1 => self.input.weapon1 = true,
                                KeyCode::Digit2 => self.input.weapon2 = true,
                                _ => {}
                            }
                        }
                    }
                    ElementState::Released => {
                        self.keys.remove(&code);
                    }
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.mouse_fire = state == ElementState::Pressed;
            }
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.mouse_fire = false;
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event
            && self.grabbed
        {
            self.input.mouse_dx += delta.0 as f32;
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
