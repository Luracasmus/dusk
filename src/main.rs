//#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#![warn(
	clippy::cargo,
	clippy::pedantic,
	clippy::nursery
)]

#![warn(
	clippy::exit,
	clippy::filetype_is_file,
	clippy::float_cmp_const,
	clippy::get_unwrap,
	clippy::integer_division,
	clippy::mem_forget,
	clippy::todo,
	clippy::unimplemented,
	clippy::unreachable,
	clippy::verbose_file_reads,
	clippy::unseparated_literal_suffix,
	clippy::unneeded_field_pattern,
	clippy::suspicious_xor_used_as_pow,
	clippy::string_to_string,
	clippy::rest_pat_in_fully_bound_structs,
	clippy::ref_patterns,
	clippy::rc_mutex,
	clippy::format_push_string,
	clippy::fn_to_numeric_cast_any,
	clippy::dbg_macro
)]

#![allow(
	clippy::cargo_common_metadata,
	clippy::multiple_crate_versions,
	clippy::cast_precision_loss,
	clippy::cast_possible_truncation,
	clippy::too_many_lines,
	clippy::cast_sign_loss
)]

mod video;

use std::num::NonZeroU32;

use ffmpeg_sidecar::download::auto_download;
use softbuffer::{Context, Surface};
use tiny_skia::{Pixmap, Color, PixmapPaint, BlendMode, Transform};
use video::{Video, Drag};
use winit::{event_loop::EventLoop, window::{WindowBuilder, Icon, Theme, CursorIcon, Fullscreen}, dpi::{LogicalSize, PhysicalPosition}, event::{Event, WindowEvent, KeyEvent, ElementState, MouseScrollDelta}, keyboard::Key};

#[derive(PartialEq, Eq)]
enum ClickState {
	Press,
	Hold,
	None
}

fn main() {
	auto_download().unwrap();

	let mut videos = vec![
		Video::import("input/sample-1.avi"),
		Video::import("input/sample-1.mov")
	];

	let mut background = Color::from_rgba8(25, 25, 35, 255);

	let mut pixmap = Pixmap::new(1, 1).unwrap();
	pixmap.fill(background);

	let event_loop = EventLoop::new();

	let window = WindowBuilder::new()
		.with_title("Dusk")
		.with_inner_size(LogicalSize::new(2560, 1440))
		.with_min_inner_size(LogicalSize::new(256, 144))
		.with_window_icon(Some(Icon::from_rgba(pixmap.data().to_vec(), pixmap.width(), pixmap.height()).unwrap()))
		.build(&event_loop)
		.unwrap();

	if let Some(theme) = window.theme() {
		background = match theme {
			Theme::Dark => Color::from_rgba8(25, 25, 35, 255),
			Theme::Light => Color::from_rgba8(225, 225, 235, 255)
		};
	}

	let context = unsafe { Context::new(&window) }.unwrap();
	let mut surface = unsafe { Surface::new(&context, &window) }.unwrap();

	let mut playhead = 0;
	let mut playing = false;

	let mut mouse_pos = PhysicalPosition { x: 0.0, y: 0.0 };
	let mut mouse_diff = PhysicalPosition { x: 0.0, y: 0.0 };
	let mut mouse_state = ClickState::None;
	let mut scroll = 0.0_f32;

	event_loop.run(move |event, _, control_flow| { match event {
		Event::MainEventsCleared => {
			if playing {
				playhead += 1;

				window.request_redraw();
			}

			for video in videos.iter_mut().rev() {
				if mouse_state == ClickState::None {
					video.drag = Drag::None;
				} else if mouse_state == ClickState::Press {
					//if (mouse_pos.x - video.x - video.scale * video.frames.first().unwrap().width) + (mouse_pos.y - video.y - video.scale * video.frames.first().unwrap().height) < 10

					let half_width = 0.5 * video.frame.width() as f32;
					let half_height = 0.5 * video.frame.height() as f32;

					// TODO: SOMETHING IS WRONG HERE WITH THE SCALE

					if (video.x + half_width - mouse_pos.x).abs() < half_width && (video.y + half_height - mouse_pos.y).abs() < half_height {
						window.set_cursor_icon(CursorIcon::Move);
						mouse_state = ClickState::Hold; // No other videos later in the video array can be grabbed

						video.drag = Drag::Move;
					}
				} else if video.drag == Drag::Move {
					video.x += mouse_diff.x;
					video.y += mouse_diff.y;

					mouse_diff = PhysicalPosition{ x: 0.0, y: 0.0 };

					if scroll.abs() > 0.01 {
						video.sx = video.sx.mul_add(scroll, video.sx);
						video.sy = video.sy.mul_add(scroll, video.sy);

						video.scaled = true;
					}

					pixmap.fill(background);

					window.request_redraw();
				}

				if video.scaled && scroll.abs() < 0.01 {
					video.resize();

					video.scaled = false;
				}
			}

			if mouse_state == ClickState::Press {
				mouse_state = ClickState::Hold;
			}

			scroll *= 0.9;
		},
		Event::RedrawRequested(_) => {
			for video in &mut videos {
				video.load(playhead - video.start);

				pixmap.draw_pixmap(
					(video.x) as i32,
					(video.y) as i32,
					video.frame.as_ref(),
					&PixmapPaint {
						blend_mode: BlendMode::Source,
						..Default::default()
					},
					Transform::from_scale(video.sx, video.sy),
					None
				);
			}

			let mut buffer = surface.buffer_mut().unwrap();

			/*for (buf, pix) in buffer.iter_mut().zip(pixmap.pixels()) {
				*buf = u32::from(pix.blue()) | u32::from(pix.green()) << 8 | u32::from(pix.red()) << 16;
			}*/

			/*for (buf, pix) in buffer.iter_mut().zip(pixmap.pixels()) {
				*buf = u32::from_ne_bytes([pix.blue(), pix.green(), pix.red(), 0]);
			}*/

			for (buf, pix) in buffer.iter_mut().zip(pixmap.data().chunks_exact(4)) {
				*buf = u32::from_le_bytes([pix[2], pix[1], pix[0], 0]);
			}

			buffer.present().unwrap();
		},
		Event::WindowEvent { event, .. } => match event {
			WindowEvent::ThemeChanged(theme) => {
				background = match theme {
					Theme::Dark => Color::from_rgba8(25, 25, 35, 255),
					Theme::Light => Color::from_rgba8(225, 225, 235, 255)
				};

				pixmap.fill(background);
				window.request_redraw();
			},
			WindowEvent::CloseRequested => control_flow.set_exit(),
			WindowEvent::KeyboardInput {
				event: KeyEvent {
					logical_key: key,
					state: ElementState::Pressed,
					..
				},
				..
			} => match key {
				Key::F11 => {
					if window.fullscreen().is_none() {
						window.set_fullscreen(Some(Fullscreen::Borderless(window.current_monitor())));
					} else {
						window.set_fullscreen(None);
					}
				},
				Key::Space => {
					if playing {
						playing = false;

						control_flow.set_wait();
					} else {
						playing = true;

						control_flow.set_poll();
					}
				},
				Key::ArrowLeft => {
					playhead = playhead.saturating_sub(1);
					window.request_redraw();
				},
				Key::ArrowRight => {
					playhead += 1;
					window.request_redraw();
				}
				_ => ()
			},
			WindowEvent::MouseInput { state, .. } => mouse_state = match state {
				ElementState::Pressed => ClickState::Press,
				ElementState::Released => {
					window.set_cursor_icon(CursorIcon::default());
					ClickState::None
				}
			},
			WindowEvent::CursorMoved { position, .. } => {
				if mouse_state == ClickState::Hold {
					mouse_diff.x += position.x as f32 - mouse_pos.x;
					mouse_diff.y += position.y as f32 - mouse_pos.y;
				}

				mouse_pos.x = position.x as f32;
				mouse_pos.y = position.y as f32;
			},
			WindowEvent::MouseWheel { delta: MouseScrollDelta::LineDelta(_, y), .. } => {
				control_flow.set_poll();

				scroll -= y * 0.0125;
			},
			WindowEvent::Resized(new_size) if new_size.width > 0 && new_size.height > 0 => {
				surface.resize(
					NonZeroU32::new(new_size.width).unwrap(),
					NonZeroU32::new(new_size.height).unwrap()
				).unwrap();

				pixmap = Pixmap::new(new_size.width, new_size.height).unwrap();
				pixmap.fill(background);
			},
			_ => ()
		},
		_ => ()
	}});
}
