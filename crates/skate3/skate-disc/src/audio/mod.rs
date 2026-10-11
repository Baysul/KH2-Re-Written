//! The skater's game audio from the user's own disc, in the layout the
//! engine's audio library reads (`private/audio/audio_manifest.json` and the
//! files it names): tools/asset_pipeline/audio_export.py `convert` in Rust,
//! for what the player's sounds use.
//!
//! Written: the rolling grains (speed bands, the whole recording and the raw
//! `.grain` member), the wheel spins, the sample banks (one WAV per stream;
//! SPLC patch trees in `patches`), the vault tuning sections (`grain_player`,
//! `player_tuning`, `bus_tuning`, `frontend`), and the native runtime's inputs
//! (Csis projects, ABKC banks, Splice trees and the MixMap) under `aems`.
//!
//! Left out (world-only): the ambience beds, map emitters, location sets,
//! zones and crossfades, region layers, the living world's banks and tuning,
//! and speech. Their manifest fields are written empty or omitted, which the
//! engine reads as an install without them.
//!
//! EA-XMA is decoded by vgmstream exactly as the Python setup runs it
//! (`vgmstream-cli -i -o ?f.wav <up to 256 names>` in the stream folder);
//! everything else is done here.
use crate::big::{Big, Entry};
use crate::{DiscFiles, Error, ReadAt, read_whole, safe_relative};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub mod formats;
pub mod tuning;

#[cfg(test)]
mod tests;

use formats::{
    bank_streams, float, grain, read_wav, scan_snr, splc_patches, splc_tree_len, standalone,
    wav_bytes, wav_info,
};
use tuning::path_stem;

/// The manifest version audio_export.py writes.
pub const MANIFEST_VERSION: u32 = 5;
/// Each rolling grain is cut into this many speed bands, each made loopable.
const GRAIN_BANDS: usize = 6;
const LOOP_CROSSFADE: f64 = 0.08; // seconds
/// vgmstream stops accepting input files somewhere between 600 and 1100
/// arguments.
const BATCH: usize = 256;
const AUDIO_ROOT: &str = "data/audio";

/// Sample banks from audiofiles.big (audio_export.py `BANKS`): the player's
/// collisions, metal, foley, whooshes, flip tricks, grinds, scrapes, squeaks,
/// skids, foot drag, shoe steps, bodyslides, rattles, seams, rolling patches,
/// sense of speed, splashes and cloth; the rolling and treatment components'
/// banks; the water lapping banks; and the front-end Splice bank.
pub const BANKS: [&str; 27] = [
    "Skate_Collisions.bnk",
    "Skate_Metal.bnk",
    "sk8_foley.bnk",
    "Sk82_Whsh_Bys.bnk",
    "Sk8_Air_Flip_Tricks.abk",
    "GRINDS.abk",
    "board_scrapes.abk",
    "Brd_Squeaks.abk",
    "WHEEL_SKID_BANK.abk",
    "FOOT_DRAG.abk",
    "fstep_skateshoe1_sm.abk",
    "Bodyslide.abk",
    "Rolling_Rattles.abk",
    "Seams_Bank.abk",
    "PatchBank_Rolling_Surfaces.abk",
    "sense_of_speed.abk",
    "water_misc.abk",
    "Foley_Cloth.abk",
    "PatchBank_SpiderCracks.abk",
    "PatchBank_Objects.abk",
    "PatchBank_RocksBounce.abk",
    "Treatments.abk",
    "water_lapping.abk",
    "water_lapping_pond.abk",
    "fountains_waterlaps_left.abk",
    "water_fountain.abk",
    "ocean_wave_small.abk",
];
/// The front-end Splice bank, also decoded (audio_export.py lists it last).
const MENU_BANK: &str = "sk8_menu.bnk";
/// Banks the native AEMS runtime needs besides the played ones.
const AEMS_EXTRA_BANKS: [&str; 2] = ["emitter_utility.abk", "Common.abk"];
const MIXMAP_FILE: &str = "MixMapSK8.mxb";

fn other(why: String) -> Error {
    Error::Io(std::io::Error::other(why))
}

/// Decodes the named files of a folder to `<name>.wav` beside them.
type Decoder<'a> = dyn FnMut(&Path, &[String]) -> Result<(), Error> + 'a;

/// Writes `root/private/audio/**` and its `audio_manifest.json` from the disc
/// (an existing `private/audio` is replaced). `collections_json` is the
/// converted skatercollections (`private/stock/skater-collections.json`, as
/// `prepare` writes it); `vgmstream` is the pinned vgmstream-cli. Work files
/// go to `root/audio-work.tmp` and are removed afterwards.
pub fn prepare_audio(
    disc: &mut dyn DiscFiles,
    collections_json: &Value,
    vgmstream: &Path,
    root: &Path,
    mut report: impl FnMut(&str),
) -> Result<(), Error> {
    let collections = collections_of(collections_json)?;
    let vgmstream = std::path::absolute(vgmstream)?;
    let mut decode = |dir: &Path, names: &[String]| run_vgmstream(&vgmstream, dir, names);
    let mut banks: Vec<&str> = BANKS.to_vec();
    banks.push(MENU_BANK);
    prepare_with(disc, collections, &banks, &mut decode, root, &mut report)
}

fn collections_of(json: &Value) -> Result<&[Value], Error> {
    json.get("collections")
        .and_then(Value::as_array)
        .or_else(|| json.as_array())
        .map(Vec::as_slice)
        .ok_or(Error::Format("collections JSON has no collections"))
}

fn prepare_with(
    disc: &mut dyn DiscFiles,
    collections: &[Value],
    banks: &[&str],
    decode: &mut Decoder<'_>,
    root: &Path,
    report: &mut dyn FnMut(&str),
) -> Result<(), Error> {
    let output = root.join("private/audio");
    let work = root.join("audio-work.tmp");
    for stale in [&output, &work] {
        if stale.exists() {
            fs::remove_dir_all(stale)?;
        }
    }
    fs::create_dir_all(&work)?;
    let result = convert(disc, collections, banks, decode, &output, &work, report);
    let cleanup = fs::remove_dir_all(&work);
    result?;
    cleanup?;
    Ok(())
}

/// A disc member's name as a single safe file name.
fn safe_name(name: &str) -> Result<&str, Error> {
    if name.contains(['/', '\\']) {
        return Err(Error::Format("unsafe archive name"));
    }
    safe_relative(name)?;
    Ok(name)
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn open_big<'d>(
    disc: &'d mut dyn DiscFiles,
    path: &str,
) -> Result<Big<Box<dyn ReadAt + 'd>>, Error> {
    let file = disc
        .open(path)?
        .ok_or_else(|| Error::Missing(path.into()))?;
    Big::open(file)
}

fn wav_entry(file: String, wav_path: &Path) -> Result<Map<String, Value>, Error> {
    let bytes = fs::read(wav_path)?;
    let wav = read_wav(&bytes)?;
    let mut entry = Map::new();
    entry.insert("file".into(), file.into());
    entry.extend(wav_info(&wav));
    Ok(entry)
}

fn convert(
    disc: &mut dyn DiscFiles,
    collections: &[Value],
    banks: &[&str],
    decode: &mut Decoder<'_>,
    output: &Path,
    work: &Path,
    report: &mut dyn FnMut(&str),
) -> Result<(), Error> {
    let mut manifest = Map::new();
    manifest.insert("version".into(), MANIFEST_VERSION.into());
    // The ambience beds are world ambience: not exported.
    manifest.insert("ambience".into(), json!({}));

    for (kind, archive, suffix) in [
        ("grains", "grains.big", ".grain"),
        ("wheels", "wheels.big", ".snr"),
    ] {
        report(&format!("Decoding {kind}"));
        let grains = kind == "grains";
        let folder = work.join(kind);
        fs::create_dir(&folder)?;
        let mut names: Vec<String> = Vec::new();
        let mut members: HashMap<String, Vec<u8>> = HashMap::new();
        {
            let mut big = open_big(disc, &format!("{AUDIO_ROOT}/{archive}"))?;
            for entry in big.entries.clone() {
                if !entry.path.ends_with(suffix) {
                    continue;
                }
                let stem = safe_name(path_stem(&entry.path))?.to_owned();
                let data = big.read(&entry)?;
                let stream = if grains {
                    grain(&data)?.stream
                } else {
                    *scan_snr(&data, 0)
                        .first()
                        .ok_or_else(|| other(format!("{} has no SNR stream", entry.path)))?
                };
                fs::write(
                    folder.join(format!("{stem}.snr")),
                    standalone(&data, &stream),
                )?;
                names.push(stem.clone());
                if grains {
                    members.insert(stem, data);
                }
            }
        }
        let files: Vec<String> = names.iter().map(|s| format!("{s}.snr")).collect();
        decode(&folder, &files)?;
        let out_dir = output.join(kind);
        fs::create_dir_all(&out_dir)?;
        let mut table = Map::new();
        for stem in &names {
            let decoded = folder.join(format!("{stem}.snr.wav"));
            if grains {
                let mut entry = Map::new();
                let bands = grain_bands(&decoded, &out_dir.join(stem), &format!("grains/{stem}"))?;
                entry.insert("bands".into(), bands);
                entry.extend(grain_whole(stem, &members[stem], &decoded, &out_dir)?);
                table.insert(stem.clone(), Value::Object(entry));
            } else {
                let target = out_dir.join(format!("{stem}.wav"));
                fs::rename(&decoded, &target)?;
                table.insert(
                    stem.clone(),
                    Value::Object(wav_entry(format!("{kind}/{stem}.wav"), &target)?),
                );
            }
        }
        manifest.insert(kind.into(), Value::Object(table));
    }

    report("Decoding sound effect banks");
    manifest.insert("banks".into(), json!({}));
    manifest.insert("patches".into(), json!({}));
    manifest.insert("grain_player".into(), tuning::grain_tuning(collections)?);
    manifest.insert("player_tuning".into(), tuning::player_tuning(collections)?);
    manifest.insert("bus_tuning".into(), tuning::bus_tuning(collections)?);
    manifest.insert("frontend".into(), tuning::frontend_sounds(collections)?);
    // World-only sections, empty (the engine treats them as absent).
    manifest.insert("emitters".into(), json!({}));
    manifest.insert("random_sets".into(), json!({}));
    manifest.insert("zones".into(), json!({}));
    manifest.insert("crossfades".into(), json!([]));
    manifest.insert("regions".into(), json!({}));
    let mut sample_banks = Map::new();
    let mut patches = Map::new();
    let mut aems = Map::new();
    {
        let mut big = open_big(disc, &format!("{AUDIO_ROOT}/audiofiles.big"))?;
        // By file name; a later entry of the same name wins, as in a dict.
        let mut by_name: HashMap<String, Entry> = HashMap::new();
        for entry in &big.entries {
            by_name.insert(basename(&entry.path).to_owned(), entry.clone());
        }
        for bank in banks {
            let entry = by_name
                .get(*bank)
                .ok_or_else(|| Error::Missing(format!("sound bank {bank}")))?
                .clone();
            let data = big.read(&entry)?;
            let stem = safe_name(path_stem(bank))?;
            let folder = work.join("banks").join(stem);
            fs::create_dir_all(&folder)?;
            let streams = bank_streams(bank, &data)?;
            let files: Vec<String> = (0..streams.len()).map(|i| format!("{i:04}.snr")).collect();
            for (file, stream) in files.iter().zip(&streams) {
                fs::write(folder.join(file), standalone(&data, stream))?;
            }
            decode(&folder, &files)?;
            let target_folder = output.join("banks").join(stem);
            fs::create_dir_all(&target_folder)?;
            let mut samples = Vec::with_capacity(streams.len());
            for index in 0..streams.len() {
                let target = target_folder.join(format!("{index:04}.wav"));
                fs::rename(folder.join(format!("{index:04}.snr.wav")), &target)?;
                samples.push(Value::Object(wav_entry(
                    format!("banks/{stem}/{index:04}.wav"),
                    &target,
                )?));
            }
            sample_banks.insert(stem.to_owned(), Value::Array(samples));
            if data.starts_with(b"SPLC") {
                // Retail's own layering/randomisation per sound.
                patches.insert(stem.to_owned(), splc_patches(&data)?);
            }
        }

        report("Copying the AEMS banks and projects");
        let aems_dir = output.join("aems");
        fs::create_dir_all(&aems_dir)?;
        // Every Csis project, in archive order (the order the runtime installs them).
        let projects: Vec<String> = big
            .entries
            .iter()
            .filter(|e| e.path.to_lowercase().ends_with(".csi"))
            .map(|e| basename(&e.path).to_owned())
            .collect();
        for name in &projects {
            let data = big.read(&by_name[name].clone())?;
            fs::write(aems_dir.join(safe_name(name)?), data)?;
        }
        aems.insert(
            "projects".into(),
            projects.iter().map(|n| format!("aems/{n}")).collect(),
        );
        // The ABKC module banks among the played ones and the extras.
        let wanted: BTreeSet<&str> = banks.iter().copied().chain(AEMS_EXTRA_BANKS).collect();
        let mut copied = BTreeMap::new();
        for bank in &wanted {
            let Some(entry) = by_name.get(*bank).cloned() else {
                continue;
            };
            if !bank.to_lowercase().ends_with(".abk") {
                continue;
            }
            let data = big.read(&entry)?;
            if !data.starts_with(b"ABKC") {
                continue;
            }
            fs::write(aems_dir.join(safe_name(bank)?), data)?;
            copied.insert(path_stem(bank).to_owned(), format!("aems/{bank}"));
        }
        aems.insert("banks".into(), json!(copied));
        // The patch tree of each played SPLC bank, for the native Splice player.
        let mut splice = BTreeMap::new();
        for bank in banks.iter().copied().collect::<BTreeSet<_>>() {
            let Some(entry) = by_name.get(bank).cloned() else {
                continue;
            };
            if !bank.to_lowercase().ends_with(".bnk") {
                continue;
            }
            let data = big.read(&entry)?;
            if !data.starts_with(b"SPLC") {
                continue;
            }
            splc_patches(&data)?; // the walk must end exactly at the sample table
            let tree = splc_tree_len(&data)?;
            let stem = safe_name(path_stem(bank))?;
            fs::write(aems_dir.join(format!("{stem}.splc")), &data[..tree])?;
            splice.insert(stem.to_owned(), format!("aems/{stem}.splc"));
        }
        aems.insert("splice".into(), json!(splice));
    }
    if let Some(path) = mixmap_file(disc, output)? {
        aems.insert("mixmap".into(), path.into());
    }
    manifest.insert("banks".into(), Value::Object(sample_banks));
    manifest.insert("patches".into(), Value::Object(patches));
    manifest.insert("aems".into(), Value::Object(aems));

    fs::write(
        output.join("audio_manifest.json"),
        formats::python_json(&Value::Object(manifest)),
    )?;
    Ok(())
}

/// Copies MixMapSK8.mxb byte for byte into `aems` (checked: big-endian header
/// with 1..64 slots and an in-file slot table); None if the disc has none.
fn mixmap_file(disc: &mut dyn DiscFiles, output: &Path) -> Result<Option<String>, Error> {
    let path = format!("{AUDIO_ROOT}/{MIXMAP_FILE}");
    if disc.open(&path)?.is_none() {
        return Ok(None);
    }
    let data = read_whole(disc, &path)?;
    if data.len() < 16 {
        return Err(other(format!("{MIXMAP_FILE}: truncated")));
    }
    let slots = u32::from_be_bytes(data[4..8].try_into().unwrap());
    let table = u32::from_be_bytes(data[8..12].try_into().unwrap());
    if !(1..=64).contains(&slots) || u64::from(table) + 4 * u64::from(slots) > data.len() as u64 {
        return Err(other(format!(
            "{MIXMAP_FILE}: not a MixMap ({slots} slots, table at {table:#x})"
        )));
    }
    fs::create_dir_all(output.join("aems"))?;
    fs::write(output.join("aems").join(MIXMAP_FILE), &data)?;
    Ok(Some(format!("aems/{MIXMAP_FILE}")))
}

/// A decoded mono grain cut into loopable speed bands (`_grain_bands`):
/// `folder/<i>.wav`, listed as `<prefix>/<i>.wav`.
fn grain_bands(source: &Path, folder: &Path, prefix: &str) -> Result<Value, Error> {
    let bytes = fs::read(source)?;
    let wav = read_wav(&bytes)?;
    if wav.sample_width != 2 || wav.channels != 1 {
        return Err(other(format!(
            "{}: expected mono PCM16 from vgmstream",
            source.display()
        )));
    }
    let samples: Vec<i16> = wav
        .data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes(*b))
        .collect();
    let crossfade = (LOOP_CROSSFADE * f64::from(wav.sample_rate)).round_ties_even() as usize;
    fs::create_dir_all(folder)?;
    let mut bands = Vec::with_capacity(GRAIN_BANDS);
    for (index, pcm) in formats::loop_bands(&samples, GRAIN_BANDS, crossfade)?
        .iter()
        .enumerate()
    {
        let target = folder.join(format!("{index}.wav"));
        fs::write(&target, wav_bytes(1, wav.sample_rate, pcm))?;
        bands.push(Value::Object(wav_entry(
            format!("{prefix}/{index}.wav"),
            &target,
        )?));
    }
    Ok(Value::Array(bands))
}

/// The whole recording for the native grain player (`grain_whole`): the raw
/// `.grain` member and its decoded PCM (moved into place).
fn grain_whole(
    stem: &str,
    member: &[u8],
    decoded: &Path,
    folder: &Path,
) -> Result<Map<String, Value>, Error> {
    let info = grain(member)?;
    fs::write(folder.join(format!("{stem}.grain")), member)?;
    let target: PathBuf = folder.join(format!("{stem}.wav"));
    fs::rename(decoded, &target)?;
    let mut entry = wav_entry(format!("grains/{stem}.wav"), &target)?;
    entry.insert("grain".into(), format!("grains/{stem}.grain").into());
    entry.insert("duration".into(), float(info.duration));
    Ok(entry)
}

/// `vgmstream-cli -i -o ?f.wav <names>` in `dir`, 256 names at a time; every
/// name must come out as `<name>.wav`.
fn run_vgmstream(exe: &Path, dir: &Path, names: &[String]) -> Result<(), Error> {
    let label = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    for batch in names.chunks(BATCH) {
        let mut command = Command::new(exe);
        command
            .args(["-i", "-o", "?f.wav"])
            .args(batch)
            .current_dir(dir)
            .stdin(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let out = command.output()?;
        if !out.status.success() {
            let mut log = String::from_utf8_lossy(&out.stdout).into_owned();
            log.push_str(&String::from_utf8_lossy(&out.stderr));
            let tail: String = log
                .chars()
                .rev()
                .take(2000)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            return Err(other(format!("vgmstream failed on {label}: {tail}")));
        }
        for name in batch {
            if !dir.join(format!("{name}.wav")).is_file() {
                return Err(other(format!("vgmstream did not decode {label}/{name}")));
            }
        }
    }
    Ok(())
}
