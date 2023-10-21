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
	clippy::cast_sign_loss,
	clippy::cognitive_complexity,
	clippy::too_many_lines,
	clippy::cast_lossless
)]

mod video;

use std::{num::NonZeroU32, time::Instant, env::current_dir};

use emath::lerp;
use ffmpeg_sidecar::download::auto_download;
use rayon_macro::parallel;
use rfd::FileDialog;
use softbuffer::{Context, Surface};
use tiny_skia::{Pixmap, Color, PixmapPaint, BlendMode, Transform, PathBuilder, Rect, Stroke, LineJoin, Paint, Shader, FillRule, NonZeroRect, Path, PixmapMut, PremultipliedColorU8};
use video::{Video, Drag};
use winit::{event_loop::{EventLoop, DeviceEvents}, window::{WindowBuilder, Icon, Theme, CursorIcon, Fullscreen}, dpi::{LogicalSize, PhysicalPosition}, event::{Event, WindowEvent, KeyEvent, ElementState, MouseScrollDelta}, keyboard::{Key, NamedKey}};

const VIDEO_EXTENSIONS: &[&str; 5] = &["webm", "mp4", "mov", "avi", "gif"];
const IMAGE_EXTENSIONS: &[&str; 4] = &["png", "jpg", "jpeg", "webp"];

#[derive(PartialEq, Eq)]
enum ClickState {
	Press,
	Hold,
	None
}

fn stroke_fill_path(
	pixmap: &mut PixmapMut,
	path: &Path,
	stroke_paint: &Paint,
	fill_paint: &Paint,
	stroke: &Stroke
) {
	pixmap.stroke_path(
		path,
		stroke_paint,
		stroke,
		Transform::identity(),
		None
	);

	pixmap.fill_path(
		path,
		fill_paint,
		FillRule::default(),
		Transform::identity(),
		None
	);
}

fn main() {
	auto_download().expect("FFmpeg could not be detected or downloaded automatically. Please install the latest FFmpeg manually");

	let mut background = Color::from_rgba8(25, 25, 35, 255);

	let event_loop = EventLoop::new().unwrap();
	event_loop.listen_device_events(DeviceEvents::Never);

	let window = {
		let mut icon = Pixmap::new(32, 32).unwrap();
		icon.fill(background);

		stroke_fill_path(
			&mut icon.as_mut(),
			&PathBuilder::from_circle(16.0, 16.0, 8.0).unwrap(),
			&Paint {
				shader: Shader::SolidColor(Color::from_rgba8(255, 134, 4, 255)),
				blend_mode: BlendMode::Source,
				..Paint::default()
			},
			&Paint {
				shader: Shader::SolidColor(Color::from_rgba8(35, 35, 55, 125)),
				..Paint::default()
			},
			&Stroke {
				width: 4.0,
				..Default::default()
			}
		);

		let w = icon.width();
		let h = icon.height();

		WindowBuilder::new()
			.with_title("Dusk")
			.with_inner_size(LogicalSize::new(2560, 1440))
			.with_min_inner_size(LogicalSize::new(256, 144))
			.with_window_icon(Some(Icon::from_rgba(icon.take(), w, h).unwrap()))
			.build(&event_loop)
			.unwrap()
	};

	window.theme().map_or_else(
		|| window.set_theme(Some(Theme::Dark)),
		|theme| {
		background = match theme {
			Theme::Dark => Color::from_rgba8(25, 25, 35, 255),
			Theme::Light => Color::from_rgba8(225, 225, 235, 255)
		};
	});

	let mut surface = {
		let context = unsafe { Context::new(&window) }.unwrap();

		unsafe { Surface::new(&context, &window) }.unwrap()
	};

	let mut videos: Vec<Video> = vec![];

	let mut playhead = 0.0;
	let mut playing = false;

	let mut mouse_pos = PhysicalPosition::new(0.0, 0.0);
	let mut mouse_diff = PhysicalPosition::new(0.0, 0.0);
	let mut mouse_state = ClickState::None;
	let mut scroll = 0.0_f32;

	let mut gui = true;
	let mut timeline = 0.0_f32;

	let mut size = window.inner_size();

	let now = Instant::now();
	let mut last_elapsed = now.elapsed().as_secs_f32();

	let mut delta = 0.0_f32;
	let mut avg = 0.0_f32;

	event_loop.run(move |event, elwt| { match event {
		Event::AboutToWait => {
			for video in videos.iter_mut().rev() {
				if mouse_state == ClickState::None {
					video.drag = Drag::None;
				} else if mouse_state == ClickState::Press {
					let half_width = 0.5 * video.frame.width() as f32;
					let half_height = 0.5 * video.frame.height() as f32;

					if (video.x + half_width - mouse_pos.x).abs() < half_width && (video.y + half_height - mouse_pos.y).abs() < half_height {
						window.set_cursor_icon(CursorIcon::Move);
						mouse_state = ClickState::Hold; // No other videos later in the video array can be grabbed

						video.drag = Drag::Move;
					}
				} else if video.drag == Drag::Move {
					video.x += mouse_diff.x;
					video.y += mouse_diff.y;

					mouse_diff = PhysicalPosition::new(0.0, 0.0);

					if scroll.abs() > 0.001 {
						let (mut sx, mut sy) = video.scale.unwrap_or((1.0, 1.0));

						sx = sx.mul_add(scroll, sx);
						sy = sy.mul_add(scroll, sy);

						video.scale = Some((sx, sy));
					}
				}

				if video.scale.is_some() && scroll.abs() < 0.001 {
					video.resize();
				}
			}

			if mouse_state == ClickState::Press {
				mouse_state = ClickState::Hold;
			}

			scroll = lerp(scroll..=0.0, (delta * 5.0).min(1.0));

			if gui {
				timeline = lerp(timeline..=1.0, (delta * 5.0).min(1.0));
			} else {
				timeline = lerp(timeline..=0.0, (delta * 5.0).min(1.0));
			}

			let visible = window.is_visible().map_or(true, |visible| visible);
			let minimized = window.is_minimized().map_or(false, |minimized| minimized);

			//println!("scroll: {scroll}");
			//println!("timeline: {timeline}");

			// && (playing || scroll.abs() > 0.001 || (timeline > 0.001 && timeline < 0.999))
			if visible && !minimized {
				window.request_redraw();
			}
		},
		Event::WindowEvent { event, .. } => match event {
			WindowEvent::RedrawRequested => {
				let new_elapsed = now.elapsed().as_secs_f32();
				delta = new_elapsed - last_elapsed;
				last_elapsed = new_elapsed;
				let fps = delta.recip();
				avg = avg.mul_add(29.0, fps) / 30.0;
				println!("{avg}");
	
				if playing { playhead += delta; }
	
				let mut buffer = surface.buffer_mut().unwrap();

				let mut pixmap = PixmapMut::from_bytes(
					bytemuck::cast_slice_mut(&mut buffer),
					size.width,
					size.height
				).unwrap();

				let mut fill = true;
	
				let occlusion: Vec<_> = videos.iter().enumerate().map(|(i, video)| {
					let x = video.x;
					let y = video.y;
					let w = video.frame.width() as f32;
					let h = video.frame.height() as f32;
	
					if
						x <= 0.0 &&
						x + w >= pixmap.width() as f32 &&
						y <= 0.0 &&
						y + h >= pixmap.height() as f32
					{
						fill = false;
					}
	
					videos[(i + 1)..].iter().any(|other| {
						other.x <= x && // left
						other.x + other.frame.width() as f32 >= x + w && // right
						other.y <= y && // top
						other.y + other.frame.height() as f32 >= y + h // bottom
					})
				}).collect();

				if fill {
					pixmap.fill(background);
				}
	
				for (video, occluded) in videos.iter_mut().zip(occlusion.into_iter()) { if !occluded {
					video.load(playhead);
	
					pixmap.draw_pixmap(
						(video.x) as i32,
						(video.y) as i32,
						video.frame.as_ref(),
						&PixmapPaint {
							blend_mode: BlendMode::Source,
							..Default::default()
						},
						if let Some((sx, sy)) = video.scale {
							NonZeroRect::from_xywh(
								video.x * (1.0 - sx),
								video.y * (1.0 - sy),
								sx,
								sy
							).map_or_else(Transform::identity, Transform::from_bbox)
						} else {
							Transform::identity()
						},
						None
					);
				}}
	
				if timeline > 0.001 {
					let scr_w = pixmap.width() as f32;
					let scr_h = pixmap.height() as f32;
	
					let menu = {
						let w = scr_w * (timeline - 0.5).max(0.0).mul_add(1.5, 0.05);
						let h = scr_h * timeline.min(0.5) * 0.15;
		
						Rect::from_xywh(
							scr_w.mul_add(0.5, w * -0.5),
							scr_h.mul_add(-0.015 * timeline, scr_h - h),
							w,
							h
						)
					};
		
					if let Some(menu) = menu {
						let alpha = 10.0 * timeline.min(0.1);
	
						let line = scr_w.min(scr_h) * 0.0025;
	
						stroke_fill_path(
							&mut pixmap,
							&PathBuilder::from_rect(menu),
							&Paint {
								shader: Shader::SolidColor(Color::from_rgba8(173, 216, 230, (alpha * 200.0) as u8)),
								..Paint::default()
							},
							&Paint {
								shader: Shader::SolidColor(Color::from_rgba8(55, 55, 85, (alpha * 125.0) as u8)),
								..Paint::default()
							},
							&Stroke {
								width: line,
								line_join: LineJoin::Round,
								..Default::default()
							}
						);
	
						let zoom = 10.0;
	
						for (i, video) in videos.iter().enumerate() {
							let preview = {
								let l = menu.left() + line;
								let t = (i as f32).mul_add(-5.0, menu.top() + line);
								let r = menu.right() - line;
								let b = (i as f32).mul_add(-5.0, menu.bottom() - line);
	
								Rect::from_ltrb(
									(l + video.duration.start() * zoom).min(r),
									t,
									(l + video.duration.end() * zoom).min(r),
									b
								)
							};
	
							if let Some(preview) = preview {
								stroke_fill_path(
									&mut pixmap,
									&PathBuilder::from_rect(preview),
									&Paint {
										shader: Shader::SolidColor(Color::from_rgba8(173, 216, 230, (alpha * 175.0) as u8)),
										..Paint::default()
									},
									&Paint {
										shader: Shader::SolidColor(Color::from_rgba8(35, 35, 55, (alpha * 100.0) as u8)),
										..Paint::default()
									},
									&Stroke {
										width: line * 0.5,
										line_join: LineJoin::Round,
										..Default::default()
									}
								);
							}
						}
					}
				}
	
				parallel!(for pix in pixmap.pixels_mut() {
					*pix = PremultipliedColorU8::from_rgba(pix.blue(), pix.green(), pix.red(), u8::MAX).unwrap();
				});
	
				window.pre_present_notify();
				buffer.present().unwrap();
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
			WindowEvent::KeyboardInput {
				event: KeyEvent {
					logical_key: key,
					state: ElementState::Pressed,
					repeat: false,
					..
				},
				..
			} => match key {
				Key::Named(key) => match key {
					NamedKey::Space => playing = !playing,
					NamedKey::Tab => gui = !gui,
					NamedKey::ArrowLeft => playhead = (playhead - 5.0).max(0.0),
					NamedKey::ArrowRight => playhead += 1.0,
					NamedKey::ArrowUp => scroll -= 0.005,
					NamedKey::ArrowDown => scroll += 0.005,
					NamedKey::F11 => window.set_fullscreen(
						if window.fullscreen().is_none() {
							Some(Fullscreen::Borderless(None))
						} else {
							None
						}
					),
					NamedKey::Delete => videos.retain_mut(|video| {
						let keep = video.drag == Drag::None;

						if !keep {
							let _ = video.ffmpeg.quit();
						}

						keep
					}),
					_ => ()
				},
				Key::Character(key) => match key.as_str() {
					"i" => {
						window.set_visible(false);

						let path = current_dir().unwrap();

						let res = FileDialog::new()
							.add_filter("Video", VIDEO_EXTENSIONS)
							.add_filter("Image", IMAGE_EXTENSIONS)
							.set_directory(path)
							.set_title("Import")
							.pick_files();

						if let Some(files) = res {
							for file in files {
								if let Some(video) = Video::new(file, playhead) {
									videos.push(video);
								}
							}
						}

						window.set_visible(true);
					}
					"e" => {
						window.set_visible(false);

						let path = current_dir().unwrap();

						let res = FileDialog::new()
							.set_file_name("dusk-export")
							.set_directory(path)
							.add_filter("Video", VIDEO_EXTENSIONS)
							.set_title("Export")
							.save_file();

						if let Some(file) = res {
							println!("{file:?}");
						}

						window.set_visible(true);
					},
					_ => ()
				},
				_ => ()
			},
			WindowEvent::Resized(new_size) if new_size.width > 0 && new_size.height > 0 => {
				surface.resize(
					NonZeroU32::new(new_size.width).unwrap(),
					NonZeroU32::new(new_size.height).unwrap()
				).unwrap();

				size = new_size;
			},
			WindowEvent::DroppedFile(path) => if let Some(video) = Video::new(path, playhead) {
				videos.push(video);
				// set video start to current playhead
			},
			WindowEvent::ThemeChanged(theme) => background = match theme {
				Theme::Dark => Color::from_rgba8(25, 25, 35, 255),
				Theme::Light => Color::from_rgba8(225, 225, 235, 255)
			},
			WindowEvent::CloseRequested => elwt.exit(),
			_ => ()
		},
		Event::LoopExiting => for video in &mut videos {
			let _ = video.ffmpeg.quit();
		},
		_ => ()
	}}).unwrap();
}
