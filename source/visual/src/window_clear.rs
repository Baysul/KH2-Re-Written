//! On Linux, each window frame starts cleared to the clear colour.
//!
//! Pinned Bevy 0.19.1 can present a window frame that nothing drew: the
//! first Wayland frame, which must be committed before any camera renders,
//! and frames whose upscaling pipeline is still compiling. Vulkan then
//! reports a frame presented in an undefined layout. Clearing each frame
//! before the render graph gives those frames the clear colour; frames the
//! cameras draw look the same. Other platforms add nothing (T177/T178).
use bevy::prelude::*;

pub struct WindowClearPlugin;

impl Plugin for WindowClearPlugin {
    #[cfg(target_os = "linux")]
    fn build(&self, app: &mut App) {
        use bevy::render::{Render, RenderApp, RenderSystems, renderer::render_system};
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render.add_systems(
                Render,
                clear_windows
                    .in_set(RenderSystems::Render)
                    .before(render_system),
            );
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn build(&self, _app: &mut App) {}
}

#[cfg(target_os = "linux")]
fn clear_windows(
    windows: Res<bevy::render::view::window::ExtractedWindows>,
    device: Res<bevy::render::renderer::RenderDevice>,
    queue: Res<bevy::render::renderer::RenderQueue>,
    clear: Res<ClearColor>,
) {
    use bevy::render::render_resource::{
        LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
    };
    let mut encoder = None;
    for view in windows
        .values()
        .filter_map(|window| window.swap_chain_texture_view.as_ref())
    {
        let encoder = encoder.get_or_insert_with(|| device.create_command_encoder(&default()));
        encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("clear window frame"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(clear.0.to_linear().into()),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    if let Some(encoder) = encoder {
        queue.submit([encoder.finish()]);
    }
}
