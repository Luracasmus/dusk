use bevy::{
    math::U16Vec2,
    prelude::*,
    window::{CompositeAlphaMode, ExitCondition, WindowResized, WindowResolution},
};
use bevy_file_dialog::FileDialogPlugin;
use ffmpeg_sidecar::command::ffmpeg_is_installed;
use rfd::MessageDialog;

use file::*;
use video::*;

mod file;
mod video;

#[derive(Clone, Copy, Default, Eq, PartialEq, Hash, Debug, States)]
enum PlayerState {
    #[default]
    Paused,
    Playing,
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    composite_alpha_mode: CompositeAlphaMode::Opaque,
                    title: "Dusk".to_string(),
                    ..Default::default()
                }),
                exit_condition: ExitCondition::OnPrimaryClosed,
                ..Default::default()
            }),
            FileDialogPlugin::new()
                .with_pick_file::<AddVideoFile>()
                .with_pick_file::<ExportVideoFile>(),
        ))
        .init_state::<PlayerState>()
        .add_systems(Startup, sys_startup)
        .add_systems(
            Update,
            (
                sys_toggle_play,
                sys_scrub,
                sys_pick_video,
                sys_export_video,
                sys_active_videos,
                sys_inactive_videos,
                sys_add_video,
                sys_playing.run_if(in_state(PlayerState::Playing)),
                sys_window_resize.run_if(on_message::<WindowResized>), // TODO: Maybe use observers instead
            ),
        )
        .init_resource::<Playhead>()
        .insert_resource(Resolution(WindowResolution::default().size().as_u16vec2()))
        .run();
}

fn sys_startup(mut commands: Commands) {
    if !ffmpeg_is_installed() {
        MessageDialog::new()
			.set_level(rfd::MessageLevel::Error)
			.set_title("FFmpeg not found")
			.set_description("Please install the latest FFmpeg or place an `ffmpeg` executable adjacent to this program")
			.show();

        // todo!() make this a file dialog that lets you choose the binary

        panic!("FFmpeg not found");
    }

    commands.spawn((
        Camera2d,
        Camera::default(),
        Projection::Orthographic(OrthographicProjection {
            /*scaling_mode: ScalingMode::AutoMin {
                min_width: 1.0,
                min_height: 1.0,
            },*/
            near: -1.0,
            far: 1.0,
            ..OrthographicProjection::default_2d()
        }),
    ));
}

fn sys_toggle_play(
    mut next_plr_state: ResMut<NextState<PlayerState>>,
    plr_state: Res<State<PlayerState>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        next_plr_state.set(match plr_state.get() {
            PlayerState::Playing => {
                println!("Paused");
                PlayerState::Paused
            }
            PlayerState::Paused => {
                println!("Playing");
                PlayerState::Playing
            }
        });
    }
}

fn sys_scrub(
    plr_state: Res<State<PlayerState>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut playhead: ResMut<Playhead>,
) {
    if keys.just_pressed(KeyCode::ArrowLeft) {
        playhead.0 = (playhead.0 - 5.0).max(0.0);
    } else if keys.pressed(KeyCode::ArrowRight) {
        playhead.0 += match plr_state.get() {
            PlayerState::Paused => 0.001,
            PlayerState::Playing => 0.5,
        };
    }
}

fn sys_playing(mut playhead: ResMut<Playhead>, time: Res<Time>) {
    playhead.0 += time.delta_secs();
}

fn sys_window_resize(
    mut resize_reader: MessageReader<WindowResized>,
    mut resolution: ResMut<Resolution>,
) {
    if let Some(new_resolution) = resize_reader.read().last() {
        resolution.0 = U16Vec2::new(new_resolution.width as u16, new_resolution.height as u16);
    }
}
