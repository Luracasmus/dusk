use std::{cmp::Ordering, path::PathBuf, num::NonZeroU16, ops::RangeInclusive};

use ffmpeg_sidecar::{child::FfmpegChild, event::OutputVideoFrame, command::FfmpegCommand};
use tiny_skia::{IntSize, Pixmap};

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
	pub frame: Option<Pixmap>,
	pub x: i32,
	pub y: i32,
	pub scale: Option<(f32, f32)>,
	pub drag: Drag,
	in_width: NonZeroU16,
	in_height: NonZeroU16,
	pub ffmpeg: FfmpegChild,
	pub duration: RangeInclusive<f32>,

	path: PathBuf,
	frame_num: u32,
	fps: f32,
	iter: Box<dyn Iterator<Item = OutputVideoFrame> + Send>
}

impl Video {
	/// Creates a new [`Video`] from a path and calls `load()` on it's first frame
	pub fn new(path: PathBuf, start: f32) -> Option<Self> {
		let mut ffmpeg = FfmpegCommand::new()
			.hide_banner()
			.create_no_window()
			.no_audio()
			.args(["-sn", "-dn"])
			.hwaccel("auto")
			.input(path.to_str().unwrap())
			.format("rawvideo")
			.pix_fmt("rgba")
			.no_overwrite()
			.pipe_stdout()
			.spawn().unwrap();

		let mut iter = ffmpeg.iter().unwrap();

		let metadata = iter.collect_metadata().unwrap();
		let stream = metadata.output_streams.first()?;

		let fps = stream.fps;

		if stream.stream_type.as_str() != "Video" || fps == 0.0 {
			print!("failed");
			return None;
		}

		Some(Self {
			in_width: NonZeroU16::new(stream.width as u16)?,
			in_height: NonZeroU16::new(stream.height as u16)?,
			frame: None,
			path,
			frame_num: 1, // To make the video reload() on first frame
			fps,
			duration: start..=start,
			x: 0,
			y: 0,
			scale: None,
			drag: Drag::None,
			ffmpeg,
			iter: Box::new(iter.filter_frames())
		})
	}

	/// Requests for the [`Video`] to load a new frame into it's `frame` field
	///
	/// * If the frame has the same timestamp as the last frame, nothing is changed
	/// * If it has a larger timestamp, `Video.iter` will advance until it reaches that timestamp
	/// * If it has a smaller timestamp, `reload()` is called on the [`Video`] and it's `ffmpeg`, `iter` and `frame` are replaced by ones starting at the requested timestamp
	pub fn load(&mut self, timestamp: f32) {
		let time = timestamp; // - self.duration.start();

		if time >= 0.0 {
			let num = (timestamp * self.fps).round() as u32;

			match num.cmp(&self.frame_num) {
				Ordering::Greater => {
					let diff = num - self.frame_num;

					let new_frame = if diff == 1 {
						self.iter.next()
					} else {
						self.iter.nth((diff - 1) as usize)
					};

					if let Some(new_frame) = new_frame {
						self.frame = Pixmap::from_vec(new_frame.data, IntSize::from_wh(new_frame.width, new_frame.height).unwrap());

						// This is not good
						if timestamp > *self.duration.end() {
							self.duration = *self.duration.start()..=timestamp;
						}
					} else {
						self.frame = None;
					}

					self.frame_num = num;
				},
				Ordering::Less => {
					// SKIP THIS IF TIMESTAMP IS OUTSIDE VIDEO

					self.frame_num = num;

					self.reload();

					if let Some(new_frame) = self.iter.next() {
						self.frame = Pixmap::from_vec(new_frame.data, IntSize::from_wh(new_frame.width, new_frame.height).unwrap());
					} else {
						self.frame = None;
					}
				},
				Ordering::Equal => ()
			}
		} else {
			self.frame = None;
			self.frame_num = 0;
		}
	}

	/// Multiplies the [`Video`]'s `in_width` and `in_height` fields by it's `scale` field, which is then set to None
	///
	/// It also advances the `iter` field by `1` frame and then `-1` frame in order to force the video to `reload()` and apply the new `width` and `height`
	pub fn resize(&mut self) {
		let (sx, sy) = self.scale.expect("Resized Video with no Scale");

		self.in_width = NonZeroU16::new(((self.in_width.get() as f32 * sx).round() as u16).max(1)).unwrap();
		self.in_height = NonZeroU16::new(((self.in_height.get() as f32 * sy).round() as u16).max(1)).unwrap();

		self.scale = None;

		self.load((self.frame_num + 1) as f32 / self.fps);
		self.load((self.frame_num - 1) as f32 / self.fps);
	}

	/// Replaces the [`Video`]'s `ffmpeg` and `iter` fields with new ones starting from `Video.timestamp`
	///
	/// This also applies changes from the `in_width` and `in_height` fields
	fn reload(&mut self) {
		drop(self.ffmpeg.quit()); // Probably not good but .unwrap() sometimes panics

		self.ffmpeg = FfmpegCommand::new()
			.hide_banner()
			.create_no_window()
			.no_audio()
			.args(["-sn", "-dn"])
			.hwaccel("auto")
			.seek((self.frame_num as f32 / self.fps).to_string())
			.input(self.path.to_str().unwrap())
			.format("rawvideo")
			.pix_fmt("rgba")
			.size(self.in_width.get() as u32, self.in_height.get() as u32)
			.no_overwrite()
			.pipe_stdout()
			.spawn().unwrap();

		self.iter = Box::new(self.ffmpeg.iter().unwrap().filter_frames());
	}
}
