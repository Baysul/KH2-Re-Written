//! Normal gameplay camera simulation. Simulation owns the cadence of
//! CameraRuntime::advance after its complete physical/animation publication.
//! skate-sim keeps only the camera numbers; Bevy camera entities, projection
//! and render postprocessing from skate-game are removed.
pub(crate) mod angle;
mod collision;
mod graph;
mod graph_conditions;
mod graph_subject;
mod publication;
mod runtime;
mod settings;
mod shake_data;
mod shot_data;
mod stock_names;
mod subject;
mod trajectory;
mod world_query;
pub(crate) use angle::CameraAngleSettings;
pub(crate) use graph_subject::CameraGraphEnvironment;
pub(crate) use publication::{
    CameraAirOutput, CameraAnimationOutput, CameraEventsOutput, CameraGrindOutput,
    CameraOffboardOutput, CameraPreferences, CameraPublicationInputs, CameraStateOutput,
    snapshot as publish_camera_subject,
};
pub(crate) use runtime::CameraRuntime;
