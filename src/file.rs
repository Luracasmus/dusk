use bevy::prelude::*;
use bevy_file_dialog::prelude::*;

use crate::video::*;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct AddVideoFile;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct ExportVideoFile;

pub fn sys_pick_video(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>) {
    if keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
        if keys.just_pressed(KeyCode::KeyO) {
            commands
                .dialog()
                .add_filter("FFmpeg-decodable video", &["mp4", "mkv", "webm"]) // todo!() query this from FFmpeg
                .pick_multiple_file_paths::<AddVideoFile>();
        } else if keys.just_pressed(KeyCode::KeyS) {
            commands
                .dialog()
                .add_filter("FFmpeg-encodable video", &["mp4", "mkv", "webm"]) // todo!() query this from FFmpeg
                .pick_multiple_file_paths::<ExportVideoFile>();
        }
    }
}

pub fn sys_add_video(
    mut commands: Commands,
    mut add: MessageReader<DialogFilePicked<AddVideoFile>>,
    playhead: Res<Playhead>,
) {
    if add.is_empty() {
        return;
    }

    for file in add.read() {
        commands.spawn((
            Video::new_inactive(file.path.clone(), playhead.0),
            Transform::default(),
        ));
        println!("Video added: {}", file.path.display());
    }
}

pub fn sys_export_video(mut export: MessageReader<DialogFilePicked<ExportVideoFile>>) {
    if export.is_empty() {
        return;
    }

    for file in export.read() {
        println!("Video export started: {}", file.path.display());
    }
}
