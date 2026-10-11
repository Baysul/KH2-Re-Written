//! The event's commands indexed by frame.
use package_reader::event::{CameraData, Command};

/// One `SeqPlayAnimation` (0x2C).
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub start: i32,
    pub end: i32,
    pub motion_start: i32,
    pub loop_start: i32,
    pub loop_end: i32,
    pub blend: i32,
    pub name: String,
}

/// One `SeqActorPosition` (0x02).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub frame: i32,
    pub position: [f32; 3],
    /// Degrees, as stored.
    pub rotation: [f32; 3],
    pub scale: f32,
}

#[derive(Clone, Debug, Default)]
pub struct ActorTrack {
    pub id: i16,
    pub object: u16,
    pub name: String,
    pub animations: Vec<Animation>,
    pub placements: Vec<Placement>,
    pub leaves: Vec<i32>,
    /// `SeqPart` (0x0D): frame and the part list.
    pub parts: Vec<(i32, Vec<i16>)>,
}

/// An actor's state at one event frame.
#[derive(Clone, Debug, PartialEq)]
pub struct ActorFrame<'a> {
    pub animation: &'a Animation,
    /// Frames since the animation started (event frames, fractional).
    pub elapsed: f32,
    pub placement: Option<Placement>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cut {
    pub camera: i16,
    pub start: i32,
    pub end: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Subtitle {
    pub frame: i32,
    pub slot: i16,
    pub message: u16,
    pub hide: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fade {
    pub frame: i32,
    pub duration: i32,
    pub kind: i16,
}

/// `AttachEffect` type 1: `child` rides `parent`'s `bone` from `frame`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attachment {
    pub frame: i32,
    pub parent: i16,
    pub child: i16,
    pub bone: i16,
}

#[derive(Clone, Debug, Default)]
pub struct Timeline {
    pub attachments: Vec<Attachment>,
    pub end_frame: i32,
    pub fade_in: i32,
    pub actors: Vec<ActorTrack>,
    pub cameras: Vec<CameraData>,
    pub cuts: Vec<Cut>,
    pub subtitles: Vec<Subtitle>,
    pub voices: Vec<(i32, String)>,
    pub fades: Vec<Fade>,
    /// (frame, flag) of `WideMask` (0x22).
    pub wide_masks: Vec<(i32, i16)>,
    /// `SetBgm` (frame, bank, bgm).
    pub set_bgm: Vec<(i32, i16, i16)>,
    /// `SeqPlayBgm` records.
    pub play_bgm: Vec<Command>,
    /// Everything else, kept for presentation layers (effects, sounds,
    /// blur, texture animation).
    pub other: Vec<Command>,
}

fn track(t: &mut Timeline, id: i16) -> &mut ActorTrack {
    let i = match t.actors.iter().position(|a| a.id == id) {
        Some(i) => i,
        None => {
            t.actors.push(ActorTrack {
                id,
                ..Default::default()
            });
            t.actors.len() - 1
        }
    };
    &mut t.actors[i]
}

impl Timeline {
    pub fn new(commands: &[Command]) -> Self {
        let mut t = Timeline::default();
        for c in commands {
            match c {
                Command::EndFrame { frame, .. } => t.end_frame = *frame as i32,
                Command::Start { fade_in, .. } => t.fade_in = *fade_in as i32,
                Command::Actor {
                    object,
                    actor,
                    name,
                } => {
                    let a = track(&mut t, *actor);
                    a.object = *object;
                    a.name = name.clone();
                }
                Command::CameraData(cam) => t.cameras.push(cam.clone()),
                Command::Camera {
                    camera, start, end, ..
                } => t.cuts.push(Cut {
                    camera: *camera,
                    start: *start as i32,
                    end: *end as i32,
                }),
                Command::Animation {
                    start,
                    end,
                    motion_start,
                    loop_start,
                    loop_end,
                    actor,
                    blend,
                    name,
                } => track(&mut t, *actor).animations.push(Animation {
                    start: *start as i32,
                    end: *end as i32,
                    motion_start: *motion_start as i32,
                    loop_start: *loop_start as i32,
                    loop_end: *loop_end as i32,
                    blend: *blend as i32,
                    name: name.clone(),
                }),
                Command::ActorPosition {
                    position,
                    rotation,
                    scale,
                    actor,
                    frame,
                    ..
                } => track(&mut t, *actor).placements.push(Placement {
                    frame: *frame as i32,
                    position: *position,
                    rotation: *rotation,
                    scale: *scale,
                }),
                Command::ActorLeave { frame, actor } => {
                    track(&mut t, *actor).leaves.push(*frame as i32)
                }
                Command::Part {
                    start,
                    actor,
                    parts,
                    ..
                } => track(&mut t, *actor)
                    .parts
                    .push((*start as i32, parts.clone())),
                Command::Subtitle {
                    frame,
                    slot,
                    message,
                    hide,
                } => t.subtitles.push(Subtitle {
                    frame: *frame as i32,
                    slot: *slot,
                    message: *message,
                    hide: *hide != 0,
                }),
                Command::Voices(v) => t
                    .voices
                    .extend(v.iter().map(|v| (v.frame as i32, v.name.clone()))),
                Command::Fade {
                    frame,
                    duration,
                    kind,
                } => t.fades.push(Fade {
                    frame: *frame as i32,
                    duration: *duration as i32,
                    kind: *kind,
                }),
                Command::AttachEffect {
                    start,
                    attach,
                    actor,
                    bone,
                    kind: 1,
                    ..
                } => t.attachments.push(Attachment {
                    frame: *start as i32,
                    parent: *attach,
                    child: *actor,
                    bone: *bone,
                }),
                Command::WideMask { frame, flag } => t.wide_masks.push((*frame as i32, *flag)),
                Command::SetBgm { frame, bank, bgm } => {
                    t.set_bgm.push((*frame as i32, *bank, *bgm))
                }
                Command::PlayBgm { .. } => t.play_bgm.push(c.clone()),
                _ => t.other.push(c.clone()),
            }
        }
        for a in &mut t.actors {
            a.animations.sort_by_key(|x| x.start);
            a.placements.sort_by_key(|x| x.frame);
            a.leaves.sort_unstable();
        }
        t.cuts.sort_by_key(|c| c.start);
        t.attachments.sort_by_key(|a| a.frame);
        t
    }

    pub fn camera(&self, id: i16) -> Option<&CameraData> {
        self.cameras.iter().find(|c| c.id == id)
    }

    /// The cut showing at `frame`: the last one started at or before it.
    pub fn cut_at(&self, frame: f32) -> Option<Cut> {
        self.cuts
            .iter()
            .rev()
            .find(|c| c.start as f32 <= frame)
            .copied()
    }

    /// The actor's animation and placement at `frame`; `None` while it is
    /// not shown (before its first animation, or after a leave that follows
    /// its latest animation start).
    pub fn actor_at(&self, id: i16, frame: f32) -> Option<ActorFrame<'_>> {
        let a = self.actors.iter().find(|a| a.id == id)?;
        let animation = a
            .animations
            .iter()
            .rev()
            .find(|x| x.start as f32 <= frame)?;
        if a.leaves
            .iter()
            .any(|&l| l > animation.start && l as f32 <= frame)
        {
            return None;
        }
        let placement = a
            .placements
            .iter()
            .rev()
            .find(|p| p.frame as f32 <= frame)
            .copied();
        Some(ActorFrame {
            animation,
            elapsed: frame - animation.start as f32,
            placement,
        })
    }

    /// The message showing in each subtitle slot at `frame`.
    pub fn subtitles_at(&self, frame: f32) -> Vec<(i16, u16)> {
        let mut slots: Vec<(i16, Option<u16>)> = Vec::new();
        for s in self.subtitles.iter().filter(|s| s.frame as f32 <= frame) {
            let value = (!s.hide).then_some(s.message);
            match slots.iter_mut().find(|(k, _)| *k == s.slot) {
                Some(e) => e.1 = value,
                None => slots.push((s.slot, value)),
            }
        }
        slots
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect()
    }
}
