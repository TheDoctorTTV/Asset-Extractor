use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs::File,
    io::{self, Read},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

pub const AUDIO_DATA_FILENAME: &str = "AudioData.dat";

pub struct ExtractedAsset {
    pub filename: String,
    pub original_filename: String,
    pub class_id: i32,
    pub payload_kind: &'static str,
}

pub struct GeneratedInfo {
    pub bytes: Vec<u8>,
    pub warnings: Vec<String>,
    pub audio_data_source: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    version: &'static str,
    song: Song,
    audio: Audio,
    song_preview_filename: String,
    cover_image_filename: String,
    environment_names: Vec<String>,
    color_schemes: Vec<Value>,
    difficulty_beatmaps: Vec<Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Song {
    title: String,
    sub_title: String,
    author: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Audio {
    song_filename: String,
    song_duration: f64,
    audio_data_filename: String,
    bpm: f64,
    lufs: f64,
    preview_start_time: f64,
    preview_duration: f64,
}

fn text(value: &Value, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|field| {
        let value = if field.starts_with('/') {
            value.pointer(field)
        } else {
            value.get(field)
        }?;
        value
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_owned)
    })
}

fn number(value: &Value, fields: &[&str]) -> Option<f64> {
    fields.iter().find_map(|field| {
        let value = if field.starts_with('/') {
            value.pointer(field)
        } else {
            value.get(field)
        }?;
        value.as_f64().filter(|v| v.is_finite())
    })
}

fn reference_key(name: &str) -> String {
    let mut name = name.to_lowercase();
    for suffix in [".asset", ".txt", ".bytes"] {
        if name.ends_with(suffix) {
            name.truncate(name.len() - suffix.len());
            break;
        }
    }
    if name.ends_with(".gz") {
        name.truncate(name.len() - 3);
    }
    name
}

fn resolve(name: &str, assets: &[ExtractedAsset]) -> Option<String> {
    // Return actual exported names, including any filename collision suffixes.
    assets
        .iter()
        .find(|asset| asset.filename.eq_ignore_ascii_case(name))
        .or_else(|| {
            assets
                .iter()
                .find(|asset| reference_key(&asset.original_filename) == reference_key(name))
        })
        .map(|asset| asset.filename.clone())
}

fn select(
    files: &[String],
    preferred: Option<String>,
    assets: &[ExtractedAsset],
    label: &str,
    warnings: &mut Vec<String>,
) -> String {
    if let Some(name) = preferred
        .and_then(|name| resolve(&name, assets))
        .filter(|name| files.contains(name))
    {
        return name;
    }
    if files.len() > 1 {
        warnings.push(format!(
            "Info.dat: multiple {label} files found; selected {}. Review this reference.",
            files[0]
        ));
    }
    files.first().cloned().unwrap_or_default()
}

fn difficulty(name: &str) -> Option<&'static str> {
    let name = name.to_lowercase().replace(['_', '-', ' ', '.'], "");
    if name.contains("expertplus") || name.contains("expert+") {
        Some("ExpertPlus")
    } else if name.contains("expert") {
        Some("Expert")
    } else if name.contains("normal") {
        Some("Normal")
    } else if name.contains("easy") {
        Some("Easy")
    } else if name.contains("hard") {
        Some("Hard")
    } else {
        None
    }
}

fn characteristic(name: &str) -> &'static str {
    let name = name.to_lowercase().replace(['_', '-', ' ', '.'], "");
    for (pattern, label) in [
        ("onesaber", "OneSaber"),
        ("noarrows", "NoArrows"),
        ("360degree", "360Degree"),
        ("90degree", "90Degree"),
        ("lawless", "Lawless"),
        ("lightshow", "Lightshow"),
    ] {
        if name.contains(pattern) {
            return label;
        }
    }
    "Standard"
}

fn lightshow(map: &str, characteristic: &str, lights: &[String]) -> String {
    let expected = map.to_lowercase().replace(".beatmap.", ".lightshow.");
    if let Some(name) = lights.iter().find(|name| name.to_lowercase() == expected) {
        return name.clone();
    }
    let map_stem = map.to_lowercase().replace(".beatmap", "");
    if let Some(name) = lights
        .iter()
        .find(|name| name.to_lowercase().replace(".lightshow", "") == map_stem)
    {
        return name.clone();
    }
    let characteristic_light = format!("{}.lightshow.dat", characteristic.to_lowercase());
    if let Some(name) = lights
        .iter()
        .find(|name| name.to_lowercase() == characteristic_light)
    {
        return name.clone();
    }
    if let Some(name) = lights
        .iter()
        .find(|name| name.eq_ignore_ascii_case("Common.lightshow.dat"))
    {
        return name.clone();
    }
    if lights.len() == 1 {
        lights[0].clone()
    } else {
        String::new()
    }
}

// AudioData sample/beat regions allow reconstruction without guessing from note timings.
fn audio_timing(data: &Value) -> (Option<f64>, Option<f64>) {
    let frequency = number(data, &["songFrequency", "_songFrequency"]).filter(|v| *v > 0.0);
    let duration = frequency
        .zip(number(data, &["songSampleCount", "_songSampleCount"]))
        .map(|(frequency, count)| count / frequency)
        .filter(|v| v.is_finite() && *v > 0.0);
    let region = data
        .get("bpmData")
        .or_else(|| data.get("_regions"))
        .and_then(Value::as_array)
        .and_then(|regions| regions.first());
    let bpm = region.and_then(|region| {
        let samples = number(region, &["ei", "_endSampleIndex"])?
            - number(region, &["si", "_startSampleIndex"])?;
        let beats = number(region, &["eb", "_endBeat"])? - number(region, &["sb", "_startBeat"])?;
        let bpm = beats * frequency? * 60.0 / samples;
        (samples > 0.0 && bpm.is_finite() && bpm > 0.0).then_some(bpm)
    });
    (duration, bpm)
}

pub fn generate(
    folder: &Path,
    bundle: &Path,
    assets: &[ExtractedAsset],
    cancel: &AtomicBool,
) -> io::Result<GeneratedInfo> {
    let mut warnings = Vec::new();
    let mut metadata = Vec::new();
    let mut maps = Vec::new();
    let mut lights = Vec::new();
    let mut audio_data = Vec::new();
    let mut audio = Vec::new();
    let mut previews = Vec::new();
    let mut covers = Vec::new();
    let mut template = Value::Null;
    let mut timings = Vec::new();
    for asset in assets {
        if cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Cancelled"));
        }
        let lower = asset.filename.to_lowercase();
        if asset.class_id == 83
            && [".wav", ".ogg", ".egg", ".mp3"]
                .iter()
                .any(|ext| lower.ends_with(ext))
        {
            if lower.contains("preview") {
                previews.push(asset.filename.clone());
            } else {
                audio.push(asset.filename.clone());
            }
        }
        if asset.class_id == 28 && lower.ends_with(".png") {
            covers.push(asset.filename.clone());
        }
        let text_asset = asset.class_id == 49 && asset.payload_kind == "text_bytes";
        if !text_asset {
            // Exported MonoBehaviour JSON can still contain useful song metadata.
            if !lower.ends_with(".json") {
                continue;
            }
        }
        let named_light = text_asset && lower.contains("lightshow");
        let named_audio_data =
            text_asset && (lower.contains("audiodata") || lower.contains("bpminfo"));
        let named_map = text_asset
            && !named_light
            && !named_audio_data
            && (lower.contains(".beatmap.") || difficulty(&lower).is_some());
        if named_map {
            maps.push(asset.filename.clone());
        }
        if named_light {
            lights.push(asset.filename.clone());
        }
        if named_audio_data {
            audio_data.push(asset.filename.clone());
        }
        // Read bounded JSON only; huge note files can still be referenced by name.
        const MAX_JSON_BYTES: u64 = 32 * 1024 * 1024;
        let file = File::open(folder.join(&asset.filename))?;
        if file.metadata()?.len() > MAX_JSON_BYTES {
            warnings.push(format!(
                "Info.dat: metadata scan skipped large file {}.",
                asset.filename
            ));
            continue;
        }
        let mut bytes = Vec::new();
        file.take(MAX_JSON_BYTES + 1).read_to_end(&mut bytes)?;
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        if !value.is_object() {
            continue;
        }
        let is_audio_data =
            value.get("songFrequency").is_some() || value.get("_songFrequency").is_some();
        if is_audio_data {
            if !named_audio_data {
                audio_data.push(asset.filename.clone());
            }
            let timing = audio_timing(&value);
            timings.push((
                asset.filename.clone(),
                timing.0,
                timing.1,
                value.pointer("/lufsData/0/l").and_then(Value::as_f64),
            ));
        } else if text_asset && !named_map && !named_light {
            if value.get("colorNotes").is_some() || value.get("_notes").is_some() {
                maps.push(asset.filename.clone());
            } else if value.get("lightColorEventBoxGroups").is_some()
                || value.get("basicBeatmapEvents").is_some()
            {
                lights.push(asset.filename.clone());
            }
        }
        if value.get("difficultyBeatmaps").is_some() && value.get("song").is_some() {
            template = value.clone();
        }
        // Retain song metadata, not the potentially large note/event arrays.
        let keys = [
            "song",
            "audio",
            "songName",
            "_songName",
            "songSubName",
            "_songSubName",
            "songAuthorName",
            "_songAuthorName",
            "songFilename",
            "_songFilename",
            "songPreviewFilename",
            "audioDataFilename",
            "coverImageFilename",
            "_coverImageFilename",
            "bpm",
            "_beatsPerMinute",
            "beatsPerMinute",
            "songDuration",
            "lufs",
            "environmentName",
            "_environmentName",
            "environmentSceneName",
            "previewStartTime",
            "_previewStartTime",
            "previewDuration",
            "_previewDuration",
        ];
        let selected = keys
            .into_iter()
            .filter_map(|key| value.get(key).map(|v| (key.to_owned(), v.clone())))
            .collect();
        metadata.push(Value::Object(selected));
    }
    if let Some(entries) = template.get("difficultyBeatmaps").and_then(Value::as_array) {
        for entry in entries {
            if let Some(name) =
                text(entry, &["beatmapDataFilename"]).and_then(|name| resolve(&name, assets))
            {
                maps.push(name);
            }
            if let Some(name) =
                text(entry, &["lightshowDataFilename"]).and_then(|name| resolve(&name, assets))
            {
                lights.push(name);
            }
        }
    }
    metadata.insert(0, template.clone());
    let find_text = |keys: &[&str]| metadata.iter().find_map(|value| text(value, keys));
    let find_number = |keys: &[&str]| metadata.iter().find_map(|value| number(value, keys));
    audio.sort_by_key(|name| {
        (
            !name.to_lowercase().contains(
                &bundle
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase(),
            ),
            name.clone(),
        )
    });
    covers.sort_by_key(|name| (!name.to_lowercase().contains("cover"), name.clone()));
    audio_data.sort_by_key(|name| (!name.eq_ignore_ascii_case("AudioData.dat"), name.clone()));
    maps.sort();
    maps.dedup();
    lights.sort();
    lights.dedup();
    audio_data.dedup();
    let song_filename = select(
        &audio,
        find_text(&["/audio/songFilename", "songFilename", "_songFilename"]),
        assets,
        "song audio",
        &mut warnings,
    );
    let preview = select(
        &previews,
        find_text(&["songPreviewFilename"]),
        assets,
        "preview audio",
        &mut warnings,
    );
    let audio_data_filename = select(
        &audio_data,
        find_text(&["/audio/audioDataFilename", "audioDataFilename"]),
        assets,
        "audio data",
        &mut warnings,
    );
    let timing = timings
        .iter()
        .find(|(name, ..)| *name == audio_data_filename);
    let duration = timing.and_then(|(_, duration, _, _)| *duration);
    let bpm = timing.and_then(|(_, _, bpm, _)| *bpm);
    let lufs = timing.and_then(|(_, _, _, lufs)| *lufs);
    let cover = select(
        &covers,
        find_text(&["coverImageFilename", "_coverImageFilename"]),
        assets,
        "cover image",
        &mut warnings,
    );
    if song_filename.is_empty() {
        warnings
            .push("Info.dat: no supported song audio was exported; songFilename is empty.".into());
    }
    if audio_data_filename.is_empty() {
        warnings.push(
            "Info.dat: no audio timing data was exported; audioDataFilename is empty.".into(),
        );
    }
    let bpm = find_number(&["/audio/bpm", "bpm", "_beatsPerMinute", "beatsPerMinute"])
        .filter(|v| *v > 0.0)
        .or(bpm)
        .unwrap_or_else(|| {
            warnings.push("Info.dat: BPM unavailable; using 120. Review before playing.".into());
            120.0
        });
    let duration = find_number(&["/audio/songDuration", "songDuration"])
        .filter(|v| *v > 0.0)
        .or(duration)
        .unwrap_or_else(|| {
            warnings.push("Info.dat: song duration unavailable; using 0 seconds.".into());
            0.0
        });
    let environments = template
        .get("environmentNames")
        .and_then(Value::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|names| !names.is_empty())
        .unwrap_or_else(|| {
            vec![find_text(&[
                "environmentName",
                "_environmentName",
                "environmentSceneName",
            ])
            .unwrap_or_else(|| {
                warnings.push(
                    "Info.dat: original environment unavailable; using DefaultEnvironment.".into(),
                );
                "DefaultEnvironment".into()
            })]
        });
    let colors = template
        .get("colorSchemes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut difficulties = Vec::new();
    let mut seen = HashSet::new();
    for map in &maps {
        let original = template
            .get("difficultyBeatmaps")
            .and_then(Value::as_array)
            .and_then(|entries| {
                entries.iter().find(|entry| {
                    text(entry, &["beatmapDataFilename"])
                        .and_then(|name| resolve(&name, assets))
                        .as_deref()
                        == Some(map.as_str())
                })
            });
        let characteristic = original
            .and_then(|entry| text(entry, &["characteristic"]))
            .unwrap_or_else(|| characteristic(map).into());
        let Some(difficulty) = original
            .and_then(|entry| text(entry, &["difficulty"]))
            .or_else(|| difficulty(map).map(str::to_owned))
        else {
            warnings.push(format!(
                "Info.dat: could not determine difficulty for {map}; no entry was guessed."
            ));
            continue;
        };
        if !seen.insert((characteristic.clone(), difficulty.clone())) {
            warnings.push(format!(
                "Info.dat: duplicate {characteristic}/{difficulty} map {map} was not referenced."
            ));
            continue;
        }
        let light = original
            .and_then(|entry| text(entry, &["lightshowDataFilename"]))
            .and_then(|name| resolve(&name, assets))
            .filter(|name| lights.contains(name))
            .unwrap_or_else(|| lightshow(map, &characteristic, &lights));
        if light.is_empty() {
            warnings.push(format!(
                "Info.dat: no unambiguous lightshow found for {map}."
            ));
        }
        let mut entry = original.cloned().unwrap_or_else(|| json!({}));
        entry["characteristic"] = json!(characteristic);
        entry["difficulty"] = json!(difficulty);
        entry["beatmapDataFilename"] = json!(map);
        entry["lightshowDataFilename"] = json!(light);
        if entry.get("beatmapAuthors").is_none() {
            entry["beatmapAuthors"] = json!({"mappers": [], "lighters": []});
        }
        let environment_idx = entry
            .get("environmentNameIdx")
            .and_then(Value::as_u64)
            .filter(|i| *i < environments.len() as u64)
            .unwrap_or(0);
        let color_idx = entry
            .get("beatmapColorSchemeIdx")
            .and_then(Value::as_u64)
            .filter(|i| *i < colors.len() as u64)
            .map(|i| i as i64)
            .unwrap_or(-1);
        entry["environmentNameIdx"] = json!(environment_idx);
        entry["beatmapColorSchemeIdx"] = json!(color_idx);
        for field in ["noteJumpMovementSpeed", "noteJumpStartBeatOffset"] {
            if entry.get(field).and_then(Value::as_f64).is_none() {
                entry[field] = json!(0.0);
            }
        }
        if original.is_none() {
            warnings.push(format!("Info.dat: inferred {map} as {characteristic}/{difficulty}; mapper credits and jump settings need review."));
        }
        difficulties.push(entry);
    }
    if difficulties.is_empty() {
        warnings.push("Info.dat: no identifiable difficulty beatmaps were exported.".into());
    }
    // The official editor copies only AudioData.dat into its working directory,
    // even if Info.dat references a differently named audio timing file.
    let audio_data_source = (!audio_data_filename.is_empty()).then_some(audio_data_filename);
    let info = Info {
        version: "4.0.1",
        song: Song {
            title: find_text(&["/song/title", "songName", "_songName"]).unwrap_or_else(|| {
                bundle
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            }),
            sub_title: find_text(&["/song/subTitle", "songSubName", "_songSubName"])
                .unwrap_or_default(),
            author: find_text(&["/song/author", "songAuthorName", "_songAuthorName"])
                .unwrap_or_default(),
        },
        audio: Audio {
            song_filename: song_filename.clone(),
            song_duration: duration,
            audio_data_filename: if audio_data_source.is_some() {
                AUDIO_DATA_FILENAME.into()
            } else {
                String::new()
            },
            bpm,
            lufs: find_number(&["/audio/lufs", "lufs"])
                .or(lufs)
                .unwrap_or(0.0),
            preview_start_time: find_number(&[
                "/audio/previewStartTime",
                "previewStartTime",
                "_previewStartTime",
            ])
            .unwrap_or(0.0)
            .max(0.0),
            preview_duration: find_number(&[
                "/audio/previewDuration",
                "previewDuration",
                "_previewDuration",
            ])
            .unwrap_or(10.0)
            .max(0.0),
        },
        song_preview_filename: if preview.is_empty() {
            song_filename
        } else {
            preview
        },
        cover_image_filename: cover,
        environment_names: environments,
        color_schemes: colors,
        difficulty_beatmaps: difficulties,
    };
    let mut bytes = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, formatter);
    info.serialize(&mut serializer).map_err(io::Error::other)?;
    bytes.push(b'\n');
    Ok(GeneratedInfo {
        bytes,
        warnings,
        audio_data_source,
    })
}
