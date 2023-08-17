use std::cmp::Ordering;

use ffmpeg_sidecar::{child::FfmpegChild, event::OutputVideoFrame, command::FfmpegCommand};
use tiny_skia::{Pixmap, IntSize};

/// Defines in what way a [`Video`] is being manipulated by the user (scale, translate, etc.)
#[derive(PartialEq, Eq)]
pub enum Drag {
	Move,
	//TopLeft,
	//TopRight,
	//BottomLeft,
	//BottomRight,
	None
}

/// Contains metadata about a specific video as well as the `FFmpeg` instance, iterator and functions required to load frames
pub struct Video {
	pub frame: Pixmap,
	pub start: u32,
	pub x: f32,
	pub y: f32,
	pub sx: f32,
	pub sy: f32,
	pub scaled: bool,
	pub drag: Drag,

	width: u32,
	height: u32,
	path: String,
	timestamp: u32,
	fps: f32,
	ffmpeg: FfmpegChild,
	iter: Box<dyn Iterator<Item = OutputVideoFrame>>
}

impl Video {
	/// Creates a new [`Video`] from a path and calls `load()` on it's first frame
	pub fn new(path: &str) -> Self {
		let mut ffmpeg = FfmpegCommand::new()
			.hide_banner()
			.create_no_window()
			.no_audio()
			.args(["-sn", "-dn"])
			.input(path)
			.format("rawvideo")
			.pix_fmt("rgba")
			.output("-")
			.spawn().unwrap();

		let mut iter = ffmpeg.iter().unwrap().filter_frames();

		let frame = iter.next().unwrap();

		let fps = {
			let next_frame = iter.next().unwrap();

			1.0 / (frame.timestamp / next_frame.timestamp)
		};

		let mut video = Self {
			frame: Pixmap::from_vec(frame.data, IntSize::from_wh(frame.width, frame.height).unwrap()).unwrap(),
			path: String::from(path),
			timestamp: 0,
			fps,
			start: 0,
			x: 0.0,
			y: 0.0,
			sx: 1.0,
			sy: 1.0,
			width: frame.width,
			height: frame.height,
			scaled: false,
			drag: Drag::None,
			ffmpeg,
			iter: Box::new(iter)
		};

		video.load(0); // Undo the iterator advancements above

		video
	}

	/// Requests for the [`Video`] to load a new frame into it's `frame` field
	///
	/// * If the frame has the same timestamp as the last frame, nothing is changed
	/// * If it has a larger timestamp, `Video.iter` will advance until it reaches that timestamp
	/// * If it has a smaller timestamp, the `reload()` is called on the [`Video`] and it's `ffmpeg`, `iter` and `frame` are replaced by ones starting at the requested timestamp
	pub fn load(&mut self, timestamp: u32) {
		match timestamp.cmp(&self.timestamp) {
			Ordering::Greater => {
				let diff = timestamp - self.timestamp;

				let new_frame = if diff == 1 {
					self.iter.next()
				} else {
					self.iter.nth(diff as usize)
				};

				if let Some(new_frame) = new_frame {
					self.frame = Pixmap::from_vec(new_frame.data, IntSize::from_wh(new_frame.width, new_frame.height).unwrap()).unwrap();
				} else {
					self.frame = Pixmap::new(1, 1).unwrap();
				}

				self.timestamp = timestamp;
			},
			Ordering::Less => {
				// SKIP THIS IF TIMESTAMP IS OUTSIDE VIDEO

				self.timestamp = timestamp;

				self.reload();

				if let Some(new_frame) = self.iter.next() {
					self.frame = Pixmap::from_vec(new_frame.data, IntSize::from_wh(new_frame.width, new_frame.height).unwrap()).unwrap();
				} else {
					self.frame = Pixmap::new(1, 1).unwrap();
				}
			},
			Ordering::Equal => ()
		}
	}

	/// Multiplies the [`Video`]'s `width` and `height` fields by it's `sx` and `sy` scale factor fields, which are then reset to `1.0`
	///
	/// It also advances the `iter` field by `1` frame and then `-1` frame in order to force the video to `reload()` and apply the new `width` and `height`
	pub fn resize(&mut self) {
		self.width = (self.width as f32 * self.sx) as u32;
		self.height = (self.height as f32 * self.sy) as u32;

		self.sx = 1.0;
		self.sy = 1.0;

		self.load(self.timestamp + 1);
		self.load(self.timestamp - 1);
	}

	/// Replaces the [`Video`]'s `ffmpeg` and `iter` fields with new ones starting from `Video.timestamp`
	///
	/// This also applies changes from the `width` and `height` fields
	fn reload(&mut self) {
		let _ = self.ffmpeg.quit(); // Probably not good but .unwrap() sometimes panics

		self.ffmpeg = FfmpegCommand::new()
			.hide_banner()
			.create_no_window()
			.no_audio()
			.args(["-sn", "-dn"])
			.seek((self.timestamp as f32 / self.fps).to_string())
			.input(&self.path)
			.format("rawvideo")
			.pix_fmt("rgba")
			.size(self.width, self.height)
			.output("-")
			.spawn().unwrap();

		self.iter = Box::new(self.ffmpeg.iter().unwrap().filter_frames());
	}
}
