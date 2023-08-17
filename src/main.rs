//#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#![warn(
	clippy::cargo,
	clippy::pedantic,
	clippy::nursery,

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
use tiny_skia::{Pixmap, Color, PixmapPaint, BlendMode, Transform, PathBuilder, Rect, Stroke, LineJoin, Paint, Shader};
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

	let mut background = Color::from_rgba8(25, 25, 35, 255);

	let event_loop = EventLoop::new().unwrap();

	let window = {
		let mut icon = Pixmap::new(1, 1).unwrap();
		icon.fill(background);

		WindowBuilder::default()
			.with_title("Dusk")
			.with_inner_size(LogicalSize::new(2560, 1440))
			.with_min_inner_size(LogicalSize::new(256, 144))
			.with_window_icon(Some(Icon::from_rgba(icon.data().to_vec(), icon.width(), icon.height()).unwrap()))
			.build(&event_loop)
			.unwrap()
	};

	if let Some(theme) = window.theme() {
		background = match theme {
			Theme::Dark => Color::from_rgba8(25, 25, 35, 255),
			Theme::Light => Color::from_rgba8(225, 225, 235, 255)
		};
	}

	let mut surface = {
		let context = unsafe { Context::new(&window) }.unwrap();

		unsafe { Surface::new(&context, &window) }.unwrap()
	};

	let mut videos = vec![
		Video::new("input/30.mp4"),
		Video::new("input/30t.gif"),
		Video::new("input/sample-1.avi")
	];

	let mut playhead = 0;
	let mut playing = false;

	let mut mouse_pos = PhysicalPosition::new(0.0, 0.0);
	let mut mouse_diff = PhysicalPosition::new(0.0, 0.0);
	let mut mouse_state = ClickState::None;
	let mut scroll = 0.0_f32;

	let mut gui = true;

	// ADD DELTATIME TO TIMELINE EVERY FRAME SO THAT PREVIEW DOESNT HAVE A SPECIFIC FPS AND MAKE IT SO THAT VIDEO LOAD GIVES THE CLOSEST FRAME TO THE CURRENT TIMESTAMP
	let mut timeline: f32 = 1.0;

	let mut pixmap = {
		let size = window.inner_size();

		Pixmap::new(size.width, size.height).unwrap()
	};

	event_loop.run(move |event, _, control_flow| { match event {
		//Event::Resumed => {},
		Event::RedrawRequested(_) => {
			pixmap.fill(background);

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

			let pix_w = pixmap.width() as f32;
			let pix_h = pixmap.height() as f32;

			let rect = if timeline > 0.5 {
				let w = (pix_w * 0.1).mul_add((timeline - 0.5).max(0.0).mul_add(6.0, timeline), 1.0);
				let h = pix_h.mul_add(0.05, 1.0);

				Rect::from_xywh(
					pix_w.mul_add(0.5, -w * 0.5),
					pix_h.mul_add(-0.01, pix_h - h),
					w,
					h
				)
			} else {
				let w = pix_w.mul_add(0.05, 1.0);
				let h = (timeline * pix_h).mul_add(0.1, 1.0);

				Rect::from_xywh(
					pix_w.mul_add(0.5, -w * 0.5),
					pix_h.mul_add(-0.01, pix_h - h),
					w,
					h
				)
			};

			if let Some(rect) = rect {
				let stroke = Stroke {
					width: pix_w.min(pix_h) * 0.01,
					line_join: LineJoin::Round,
					..Default::default()
				};

				let path = PathBuilder::from_rect(rect);

				pixmap.stroke_path(
					&path,
					&Paint { shader: Shader::SolidColor(Color::from_rgba8(173, 216, 230, 200)), ..Paint::default() },
					&stroke,
					Transform::default(),
					None
				);
			}

			let mut buffer = surface.buffer_mut().unwrap();

			for (buf, pix) in buffer.iter_mut().zip(pixmap.data().chunks_exact(4)) {
				*buf = u32::from_le_bytes([pix[2], pix[1], pix[0], 0]);
			}

			window.pre_present_notify();
			//buffer.present_with_damage(&[softbuffer::Rect { x: 0, y: 0, width: NonZeroU32::new(1).unwrap(), height: NonZeroU32::new(1).unwrap() }]).unwrap();
			buffer.present().unwrap();
		},
		Event::AboutToWait => {
			if playing {
				playhead += 1;
			}

			for video in videos.iter_mut().rev() {
				if mouse_state == ClickState::None {
					video.drag = Drag::None;
				} else if mouse_state == ClickState::Press {
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

					mouse_diff = PhysicalPosition::new(0.0, 0.0);

					if scroll.abs() > 0.01 {
						video.sx = video.sx.mul_add(scroll, video.sx);
						video.sy = video.sy.mul_add(scroll, video.sy);

						video.scaled = true;
					}
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

			if gui {
				timeline = (timeline + 0.05).min(1.0);
			} else {
				timeline *= 0.9;
			}

			let visible = window.is_visible().map_or(true, |visible| visible);
			let minimized = window.is_minimized().map_or(false, |minimized| minimized);

			if visible && !minimized {
				window.request_redraw();
			}
		},
		Event::WindowEvent { event, .. } => match event {
			WindowEvent::ThemeChanged(theme) => background = match theme {
				Theme::Dark => Color::from_rgba8(25, 25, 35, 255),
				Theme::Light => Color::from_rgba8(225, 225, 235, 255)
			},
			WindowEvent::CloseRequested => control_flow.set_exit(),
			WindowEvent::KeyboardInput {
				event: KeyEvent {
					logical_key: key,
					state: ElementState::Pressed,
					repeat: false,
					..
				},
				..
			} => match key {
				Key::F11 => window.set_fullscreen(
					if window.fullscreen().is_none() {
						Some(Fullscreen::Borderless(window.current_monitor()))
					} else {
						None
					}
				),
				Key::Space => playing = !playing,
				Key::Tab => gui = !gui,
				Key::ArrowLeft => playhead = playhead.saturating_sub(1),
				Key::ArrowRight => playhead += 1,
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
			WindowEvent::MouseWheel { delta: MouseScrollDelta::LineDelta(_, y), .. } => scroll -= y * 0.0125,
			WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
				surface.resize(
					NonZeroU32::new(size.width).unwrap(),
					NonZeroU32::new(size.height).unwrap()
				).unwrap();

				pixmap = Pixmap::new(size.width, size.height).unwrap();
			},
			_ => ()
		},
		_ => ()
	}}).unwrap();
}
