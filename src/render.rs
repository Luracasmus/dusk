use rayon_macro::parallel;
use tiny_skia::{BlendMode, Color, FilterQuality, NonZeroRect, PixmapMut, PixmapPaint, Transform};

use crate::video::Video;

/// Clears the buffer and loads and draws all [`Video`]s to it
pub fn render_frame(pixmap: &mut PixmapMut, videos: &mut [Video], playhead: f32, background: Color) {
	parallel!(for video in &mut *videos {
		video.load(playhead);
	});

	let mut fill = true;

	#[allow(clippy::needless_collect)] // Required for `fill`
	let occlusion: Vec<_> = videos.iter().enumerate().map(|(i, video)| {
		video.frame.as_ref().map_or(true, |frame| {
			let x = video.x;
			let y = video.y;
			let w = frame.width() as i32;
			let h = frame.height() as i32;

			if
				x <= 0 &&
				x + w >= pixmap.width() as i32 &&
				y <= 0 &&
				y + h >= pixmap.height() as i32
			{
				fill = false;
			}

			videos[(i + 1)..].iter().any(|other| {
				other.frame.as_ref().map_or(false, |other_frame|
					other.x <= x && // left
					other.x + other_frame.width() as i32 >= x + w && // right
					other.y <= y && // top
					other.y + other_frame.height() as i32 >= y + h // bottom
				)
			})
		})
	}).collect();

	if fill {
		pixmap.fill(background);
	}

	for (video, occluded) in videos.iter().zip(occlusion.into_iter()) { if !occluded {
		if let Some(frame) = &video.frame {
			pixmap.draw_pixmap(
				video.x,
				video.y,
				frame.as_ref(),
				&PixmapPaint {
					blend_mode: BlendMode::Source,
					quality: FilterQuality::Bilinear, // Severe performance impact while resizing videos
					..Default::default()
				},
				if let Some((sx, sy)) = video.scale {
					NonZeroRect::from_xywh(
						video.x as f32 * (1.0 - sx),
						video.y as f32 * (1.0 - sy),
						sx,
						sy
					).map_or_else(Transform::identity, Transform::from_bbox)
				} else {
					Transform::identity()
				},
				None
			);
		}
	}}
}