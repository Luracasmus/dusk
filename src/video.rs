use bevy::{
    asset::RenderAssetUsages,
    math::{U16Vec2, UVec2},
    platform::cell::SyncCell,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use ffmpeg_sidecar::{
    child::FfmpegChild,
    command::FfmpegCommand,
    event::{OutputVideoFrame, StreamTypeSpecificData},
};

use std::{
    cmp::Ordering,
    num::NonZero,
    ops::Range,
    path::{Path, PathBuf},
};

// Wrapper to make the FFmpeg child process quit gracefully on drop
struct FFmpegWrapper(FfmpegChild);

impl Drop for FFmpegWrapper {
    fn drop(&mut self) {
        if let Err(err) = self.0.quit() {
            println!(
                "Failed to request FFmpeg child process to gracefully quit: {err}\nKilling it instead"
            );

            self.0.kill().expect("Failed to kill FFmpeg child process");
        }

        // We skip waiting and just hope the process always quits
        // self.0.wait().unwrap();
    }
}

struct Decoder {
    frame: u32,
    fps: f32,
    width: NonZero<u16>,
    height: NonZero<u16>,
    iter: SyncCell<Box<dyn Iterator<Item = OutputVideoFrame> + Send>>,
    _ffmpeg: FFmpegWrapper, // The field order here (determines drop order) seems to be important for the FFmpeg child process to quit properly
}

impl Decoder {
    #[must_use]
    fn new(path: &Path, seek: f32, size: UVec2) -> Option<(Self, Vec<u8>)> {
        let mut command = FfmpegCommand::new();
        command
            .hide_banner()
            .create_no_window()
            .no_audio()
            .args(["-sn", "-dn"])
            .hwaccel("auto");

        if seek != 0.0 {
            command.seek(seek.to_string());
        }

        // todo!() look into .duration and .readrate

        let mut ffmpeg = command
            .input(path.to_str().unwrap())
            .format("rawvideo")
            .pix_fmt("rgba") // todo!() let FFmpeg pick this automatically and choose the Image format accordingly, reconstructing the Decoder if none of the formats match
            .size(size.x, size.y)
            .no_overwrite()
            .pipe_stdout()
            .spawn()
            .unwrap();

        let mut iter = ffmpeg.iter().unwrap();

        let metadata = iter.collect_metadata().unwrap();
        let stream = metadata.output_streams.first()?; // is the video always the first stream?

        if let StreamTypeSpecificData::Video(video_stream) = &stream.type_specific_data {
            let mut frame_iter = iter.filter_frames();
            let first_frame = frame_iter.next()?;

            Some((
                Self {
                    _ffmpeg: FFmpegWrapper(ffmpeg),
                    iter: SyncCell::new(Box::new(frame_iter)),
                    frame: (seek * video_stream.fps) as u32,
                    fps: video_stream.fps,
                    width: NonZero::new(video_stream.width as u16)?,
                    height: NonZero::new(video_stream.height as u16)?,
                },
                first_frame.data,
            ))
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Resource)]
pub struct Playhead(pub f32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Resource)]
pub struct Resolution(pub U16Vec2);

#[derive(Component)]
pub struct Video {
    pub duration: Range<f32>,
    pub shift: f32, // 0..
    pub size: Vec2, // 0..=1.0
    pub source: PathBuf,
    decoder: Option<Decoder>,
}

impl Video {
    #[must_use]
    pub const fn new_inactive(source: PathBuf, start: f32) -> Self {
        Self {
            duration: start..f32::INFINITY,
            shift: 0.0,
            size: Vec2::ONE,
            source,
            decoder: None,
        }
    }
}

pub fn sys_inactive_videos(
    mut commands: Commands,
    mut inactive_videos: Query<(Entity, &mut Video), Without<Sprite>>,
    mut images: ResMut<Assets<Image>>,
    playhead: Res<Playhead>,
    resolution: Res<Resolution>,
) {
    for (entity, mut video) in &mut inactive_videos {
        if video.duration.contains(&playhead.0) {
            let scaled_size = (video.size * resolution.0.as_vec2()).as_uvec2();

            if let Some((new_decoder, first_frame)) =
                Decoder::new(&video.source, playhead.0, scaled_size)
            {
                println!(
                    "Made video active {{ source: {}, duration: {:?}, shift: {}, size: {}, Decoder {{ fps: {}, frame: {}, width: {}, height: {} }} }}",
                    video.source.display(),
                    video.duration,
                    video.shift,
                    video.size,
                    new_decoder.fps,
                    new_decoder.frame,
                    new_decoder.width,
                    new_decoder.height
                );
                video.decoder = Some(new_decoder);

                commands
                    .entity(entity)
                    .insert(Sprite::from_image(images.add(Image::new(
                        Extent3d {
                            width: scaled_size.x,
                            height: scaled_size.y,
                            depth_or_array_layers: 1,
                        },
                        TextureDimension::D2,
                        first_frame,
                        TextureFormat::Rgba8UnormSrgb,
                        RenderAssetUsages::default(),
                    ))));
            } else {
                println!("Failed to create decoder: {}", video.source.display());
                todo!();
            }
        }
    }
}

pub fn sys_active_videos(
    mut commands: Commands,
    mut active_videos: Query<(Entity, &mut Video, &Sprite)>,
    mut images: ResMut<Assets<Image>>,
    playhead: Res<Playhead>,
    resolution: Res<Resolution>,
) {
    for (entity, mut video, sprite) in &mut active_videos {
        let duration = video.duration.clone();
        let shift = video.shift;
        let source = video.source.clone();
        let size = video.size;

        if let Some(decoder) = &mut video.decoder {
            if duration.contains(&playhead.0) {
                let requested_frame = ((playhead.0 - shift) * decoder.fps) as u32;

                match requested_frame.cmp(&decoder.frame) {
                    Ordering::Equal => (),
                    Ordering::Greater => {
                        let diff = requested_frame - decoder.frame;
                        decoder.frame = requested_frame;

                        let single_frame = diff == 1;
                        let step = (diff - 1) as usize;

                        let new_frame = {
                            let iter = decoder.iter.get();

                            if single_frame {
                                iter.next()
                            } else {
                                iter.nth(step)
                            }
                        };

                        println!(
                            "Playing video: {{ source: {}, requested_frame: {requested_frame} diff: {diff} }}",
                            source.display()
                        );

                        if let Some(new_frame) = new_frame {
                            images.get_mut(sprite.image.id()).unwrap().data = Some(new_frame.data);
                        } else {
                            todo!(); // return a completely red frame or something, to warn the user
                            // video.duration.end = playhead.0; // is this jank?
                        }
                    }
                    Ordering::Less => {
                        if let Some((new_decoder, first_frame)) = Decoder::new(
                            &source,
                            playhead.0,
                            (size * resolution.0.as_vec2()).as_uvec2(),
                        ) {
                            *decoder = new_decoder;
                            images.get_mut(sprite.image.id()).unwrap().data = Some(first_frame);
                        } else {
                            todo!()
                            // something has gone very wrong
                        }
                    }
                }
            } else {
                println!("Made video inactive: {}", video.source.display());

                video.decoder = None;
                images.remove(sprite.image.id());
                commands.entity(entity).remove::<Sprite>();
            }
        }
    }
}
