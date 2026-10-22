//! Research probe: runs a party limit's scene program (limit/<lang>/<name>.bar)
//! headless on the object-script engine, with clip lengths and trigger tables
//! read from the limit file's own motions, on a flat floor. It presses the
//! reaction button on every command the scene offers and prints what the
//! scene asked for: motions, effects, services, missing traps and faults.
//! Prints names and numbers only.
use data::{limit_file, motion, path_lookup::Library, probe_source::ProbeSource};
use enemy::{
    env::{Clips, Env},
    limit_scene::{Participant, Scene, Status},
    motion::{FrameEvent, MotionEvents, RangeEvent},
    sysobj::Sysobj,
    traps::limit::MOTIONS,
    world::Event,
};
use std::{collections::HashMap, path::PathBuf};

type Vec3 = [f32; 3];

#[derive(Default)]
struct ClipSet(HashMap<usize, (u32, u32, MotionEvents)>);

impl Clips for ClipSet {
    fn length(&self, slot: usize) -> u32 {
        self.0.get(&slot).map_or(0, |c| c.0)
    }
    fn loop_start(&self, slot: usize) -> u32 {
        self.0.get(&slot).map_or(0, |c| c.1)
    }
    fn root_delta(&self, _: usize, _: u32) -> Vec3 {
        [0.; 3]
    }
    fn events(&self, slot: usize) -> Option<&MotionEvents> {
        self.0.get(&slot).map(|c| &c.2)
    }
}

struct Flat {
    clips: Vec<ClipSet>,
    enemy: Vec3,
}

impl Env for Flat {
    fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
        Some(0.)
    }
    fn check_step(&self, _: Vec3, _: Vec3, _: f32, _: f32, _: f32) -> Option<u8> {
        Some(0)
    }
    fn clips(&self, model: u32) -> &dyn Clips {
        &self.clips[(model as usize).min(self.clips.len() - 1)]
    }
    fn player_position(&self) -> Vec3 {
        [0.; 3]
    }
    fn target_position(&self, id: u32) -> Option<Vec3> {
        (id == 2).then_some(self.enemy)
    }
    fn lockon_target(&self) -> Option<u32> {
        Some(enemy::traps::target::OBJECT_TARGET | 2)
    }
    fn pick_target(&self, _: i32, _: Vec3) -> Option<u32> {
        Some(2)
    }
}

fn clip_of(anb: &[u8]) -> Result<(u32, u32, MotionEvents), Box<dyn std::error::Error>> {
    let m = motion::read_anb(anb, 0)?;
    let (start, end, fps, ret) = m.motion.frame_window()?;
    let scale = 60. / fps;
    let begin = (start.max(0.) * scale).round() as u32;
    let length = ((end * scale).round() as u32).saturating_sub(begin);
    let loop_start = ((ret * scale).round() as u32).saturating_sub(begin);
    let events = MotionEvents {
        ranges: m
            .triggers
            .iter()
            .flat_map(|t| &t.records.ranges)
            .map(|r| RangeEvent {
                start: r.start_frame as u16,
                end: r.end_frame as u16,
                kind: r.trigger,
                params: r.parameters.iter().map(|&v| v as u16).collect(),
            })
            .collect(),
        frames: m
            .triggers
            .iter()
            .flat_map(|t| &t.records.frames)
            .map(|f| FrameEvent {
                frame: f.frame as u16,
                kind: f.trigger,
                params: f.parameters.iter().map(|&v| v as u16).collect(),
            })
            .collect(),
    };
    Ok((length, loop_start, events))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 6 {
        return Err("usage: limit_probe <Steam-root> <manifest> <limit name, e.g. goofy> <command id> <friend characters, e.g. 3 or 2,3> <ticks>".into());
    }
    let source = ProbeSource::bind(&args[0], &args[1])?;
    let mut library = Library::open(&source)?;
    let name = args[2].to_string_lossy().into_owned();
    let command: u16 = args[3].to_string_lossy().parse()?;
    let friends: Vec<u8> = args[4]
        .to_string_lossy()
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    let ticks: u32 = args[5].to_string_lossy().parse()?;
    // LIMIT_PROBE_TRAPS=t:i,t:i,...: those the engine has no routine for.
    if let Ok(list) = std::env::var("LIMIT_PROBE_TRAPS") {
        let missing: Vec<&str> = list
            .split(',')
            .filter(|t| {
                let mut p = t.split(':').filter_map(|v| v.trim().parse::<u16>().ok());
                match (p.next(), p.next()) {
                    (Some(a), Some(b)) => enemy::traps::lookup(a, b).is_none(),
                    _ => false,
                }
            })
            .collect();
        println!("traps missing {} {}", missing.len(), missing.join(","));
    }
    // LIMIT_PROBE_SUMMONS=1: 00battle `sumn` rows, the `limt` rows whose
    // second member is a summon (200..204), and the summon commands.
    if std::env::var_os("LIMIT_PROBE_SUMMONS").is_some() {
        let entry = |file: &[u8], name: &[u8; 4]| -> Option<Vec<u8>> {
            let bar = data::bar::read_bar(file).ok()?;
            let e = bar.entries.iter().find(|e| &e.name_bytes == name)?;
            file.get(e.offset as usize..e.offset as usize + e.size as usize)
                .map(<[u8]>::to_vec)
        };
        let battle = library.read("00battle.bin")?;
        let system = library.read("03system.bin")?;
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        let mut ids = vec![0x48u16, 0x13c];
        if let Some(sumn) = entry(&battle, b"sumn") {
            for row in sumn.get(8..).unwrap_or(&[]).chunks(0x40) {
                println!("sumn {}", hex(row));
                ids.push(u16::from_le_bytes([row[0], row[1]]));
            }
        }
        if let Some(limt) = entry(&battle, b"limt") {
            for row in limt.get(8..).unwrap_or(&[]).chunks(0x40) {
                if (200..=204).contains(&row[2]) {
                    println!("limt {}", hex(row));
                    ids.push(u16::from_le_bytes([row[0x28], row[0x29]]));
                }
            }
        }
        if let Some(cmd) = entry(&system, b"cmd\0") {
            let commands = data::command_table::read(&cmd)?;
            for c in commands.iter().filter(|c| ids.contains(&c.id)) {
                println!("{c:?}");
            }
        }
    }
    // LIMIT_PROBE_SCRIPT=<model> with LIMIT_PROBE_SCRIPT_OUT=<file>: the
    // model's object script (BAR type 3), for private listing.
    if let (Ok(model), Some(out)) = (
        std::env::var("LIMIT_PROBE_SCRIPT"),
        std::env::var_os("LIMIT_PROBE_SCRIPT_OUT"),
    ) {
        let bytes = library.read(&format!("obj/{model}.mdlx"))?;
        let bar = data::bar::read_bar(&bytes)?;
        if let Some(e) = bar
            .entries
            .iter()
            .find(|e| e.entry_type == 3 && e.size != 0)
        {
            let at = e.offset as usize;
            std::fs::write(out, &bytes[at..at + e.size as usize])?;
        }
    }
    // LIMIT_PROBE_OBJ=<hex objentry id>: that object's model and motion names.
    if let Ok(id) = std::env::var("LIMIT_PROBE_OBJ")
        && let Ok(id) = u32::from_str_radix(id.trim_start_matches("0x"), 16)
    {
        let objects = data::objentry::read(&library.read("00objentry.bin")?)?;
        for o in objects.iter().filter(|o| o.object_id == id) {
            println!(
                "object {id:#x} type {} model {} motion {}",
                o.object_type,
                String::from_utf8_lossy(&o.model_name),
                String::from_utf8_lossy(&o.animation_name)
            );
        }
    }
    let file = limit_file::read(&library.read(&format!("limit/us/{name}.bar"))?)?;
    println!(
        "program {} bytes, motions {:?}, pax {}, cameras {}, sheets {}",
        file.program.len(),
        file.motion_ids(),
        file.pax.as_ref().map_or(0, Vec::len),
        file.cameras.len(),
        file.sheets.len()
    );
    // The effect elements: id, effect number, end time and looping.
    if let Some(pax) = file
        .pax
        .as_ref()
        .and_then(|b| data::object_effect::read_pax(b).ok())
    {
        for e in &pax.elements {
            let pdata = pax.effect(e.effect_number).and_then(|x| {
                let d = pax.dpds.get(x.dpd)?.pdata.get(x.pdata)?;
                Some((d.end_time, d.looping, d.entries.len()))
            });
            println!(
                "  element {} effect {} flags {:#x} bone {} fadeout {} pdata {:?}",
                e.id, e.effect_number, e.flags, e.bone, e.fadeout_frame, pdata
            );
        }
    }
    // LIMIT_PROBE_SHEET=<file>: a sheet's HD texture as raw RGBA (width and
    // height in the file name), for private inspection; LIMIT_PROBE_SHEET_NAME
    // names the sheet (default `gage`).
    let sheet_name: [u8; 4] = std::env::var("LIMIT_PROBE_SHEET_NAME")
        .ok()
        .and_then(|n| n.as_bytes().try_into().ok())
        .unwrap_or(*b"gage");
    if let Some(out) = std::env::var_os("LIMIT_PROBE_SHEET")
        && let Some(found) = library.find(&format!("limit/us/{name}.bar"))
        && let Ok(sheet) = visual::hud::load(
            &source,
            found.index,
            found.ordinal,
            found.name_hash,
            &sheet_name,
        )
    {
        let mut path = PathBuf::from(out);
        path.set_extension(format!(
            "{}x{}.rgba",
            sheet.texture_size[0], sheet.texture_size[1]
        ));
        std::fs::write(&path, &sheet.texture_rgba)?;
        println!("  texel scale {}", sheet.texel_scale);
    }
    // The gauge sheets: each group's animations.
    for (name, _, seq) in &file.sheets {
        if let Ok(seq) = data::sprite_sequence::read(seq) {
            println!(
                "  sheet {} groups {}",
                String::from_utf8_lossy(name),
                seq.groups.len()
            );
            for (g, group) in seq.groups.iter().enumerate() {
                let anims: Vec<_> = seq.animations[group.animations.clone()]
                    .iter()
                    .map(|a| {
                        (
                            format!("{:#x}", a.flags),
                            a.sprite_group,
                            a.frames,
                            a.translate,
                            a.scale,
                            a.scale_x,
                            format!("{:08x}/{:08x}", a.color[0], a.color[1]),
                        )
                    })
                    .collect();
                if String::from_utf8_lossy(name) == "gage" && (g == 3 || g == 0xc || g == 9) {
                    let mut pl = data::sprite_sequence::Player::new(&seq, g);
                    pl.update(&seq, 1.);
                    let m = data::sprite_sequence::Modifiers::default();
                    let ctx = seq.child_context(&pl, &m);
                    for q in seq.quads(&pl, [0., 0.], &m) {
                        println!(
                            "      quad {:?} uv {:?} col {:?} blend {}",
                            q.corners, q.uv, q.colors[0], q.blend
                        );
                    }
                    println!("      context {ctx:?}");
                }
                println!(
                    "    group {g:#x} play {} loop {:?} light {} texty {} pad {} textx {} {:?}",
                    group.play_count,
                    group.loop_frames,
                    group.light_position_x,
                    group.text_position_y,
                    group.ui_padding,
                    group.text_position_x,
                    anims
                );
            }
        }
    }
    // Model 0 Sora, model k the friend of character k; their weapons are
    // model 16 + character.
    let mut clips: Vec<ClipSet> = (0..32).map(|_| ClipSet::default()).collect();
    let bodies =
        std::iter::once((0usize, 1u8, None)).chain(friends.iter().map(|&c| (c as usize, c, None)));
    let weapons = std::iter::once((17usize, 1u8, Some(0u8)))
        .chain(friends.iter().map(|&c| (16 + c as usize, c, Some(0u8))));
    for (model, character, weapon) in bodies.chain(weapons) {
        let set = &mut clips[model];
        set.0.insert(0, (60, 0, MotionEvents::default()));
        for id in file.motion_ids() {
            if let Some(anb) = file.anb(id, limit_file::motion_key(character, weapon)) {
                let c = clip_of(&anb)?;
                println!(
                    "  char {character} weapon {weapon:?} motion {id}: {} ticks, loop {}, ranges {:?}, frames {:?}",
                    c.0,
                    c.1,
                    c.2.ranges
                        .iter()
                        .map(|r| (r.kind, r.start, r.end))
                        .collect::<Vec<_>>(),
                    c.2.frames
                        .iter()
                        .map(|f| (f.kind, f.frame))
                        .collect::<Vec<_>>()
                );
                set.0.insert(MOTIONS + id as usize, c);
            }
        }
    }
    let env = Flat {
        clips,
        enemy: [0., 0., 300.],
    };
    let weapon = |owner: u32, model: u32| {
        let mut s = Sysobj::bare();
        s.model = model;
        s.attached = Some(enemy::sysobj::Attach {
            to: owner,
            bone: 0,
            flags: 0,
            callback: None,
        });
        s
    };
    let mut sora = Sysobj::bare();
    sora.model = 0;
    sora.weapons = vec![30];
    let mut others = vec![Participant {
        id: 30,
        sys: weapon(enemy::world::PLAYER_OBJECT, 17),
    }];
    let friends: Vec<Participant> = friends
        .iter()
        .enumerate()
        .map(|(k, &c)| {
            let mut s = Sysobj::bare();
            s.model = c as u32;
            s.position = [100. * (k as f32 + 1.), 0., 0.];
            let id = 10 + k as u32;
            s.weapons = vec![31 + k as u32];
            others.push(Participant {
                id: 31 + k as u32,
                sys: weapon(id, 16 + c as u32),
            });
            Participant { id, sys: s }
        })
        .collect();
    let mut enemy = Sysobj::bare();
    enemy.position = env.enemy;
    others.push(Participant { id: 2, sys: enemy });
    let mut scene = Scene::start(&env, &file.program, command, 180., sora, friends, others)?;
    let mut offered: Option<(u32, u16)> = None;
    for tick in 0..ticks {
        let status = scene.tick(&env);
        for e in scene.take_events() {
            match &e {
                Event::Service {
                    name: "command_override",
                    words,
                    ..
                } => {
                    let cmd = words[1] as u16;
                    if offered.is_none_or(|o| o.1 != cmd || tick > o.0 + 25) {
                        println!("{tick}: offer {cmd:#x} {:x?}", &words[..4]);
                        offered = Some((tick, cmd));
                    }
                }
                Event::Service {
                    name,
                    words,
                    object,
                    ..
                } => {
                    println!("{tick}: service {name} {:?} {:x?}", object, &words[..4])
                }
                Event::EffectStart {
                    object,
                    effect,
                    other,
                    ..
                } => {
                    println!("{tick}: effect {effect} on {object:#x} bound {other:?}")
                }
                Event::MotionEvent {
                    object,
                    range,
                    kind,
                    params,
                } => {
                    if !*range {
                        println!("{tick}: frame event {kind} on {object:#x} {params:?}");
                    }
                }
                Event::MotionEffect { object, id, .. } => {
                    println!("{tick}: motion effect {id} on {object:#x}")
                }
                Event::Presentation { name, words } => {
                    println!("{tick}: presentation {name} {words:x?}")
                }
                other => println!("{tick}: {other:?}"),
            }
        }
        let stop = std::env::var("LIMIT_PROBE_STOP")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(u32::MAX);
        if let Some((at, cmd)) = offered
            && tick == at + 20
            && tick < stop
        {
            println!("{tick}: follow-up {cmd:#x}");
            scene.follow_up(&env, cmd);
        }
        if tick % 30 == 0 {
            let l = scene.limit();
            println!(
                "{tick}: time {:.1} pending {} sora slot {} tick {} pos {:?}",
                l.time,
                l.pending,
                scene.player().motion.slot,
                scene.player().motion.tick,
                scene.player().position
            );
        }
        if status == Status::Over {
            println!("{tick}: over");
            break;
        }
    }
    println!("missing traps {:?}", scene.missing());
    println!("faults {:?}", scene.faults());
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
