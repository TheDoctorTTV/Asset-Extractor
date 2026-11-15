use serde_json::Value;
use std::{
    collections::HashMap,
    io,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use unity_rs_core::{
    loader::AssetCollection,
    monobehaviour::{read_mono_behaviour, read_mono_behaviour_json, MonoBehaviourReadLimits},
    serialized::SerializedFile,
};

#[derive(Default)]
pub struct Environments {
    default: Option<String>,
    all_directions: Option<String>,
    difficulties: HashMap<(String, String), String>,
}

fn name(value: &Value) -> Option<String> {
    value
        .as_str()
        .or_else(|| value.get("_environmentName")?.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
}

fn basename(path: &str) -> &str {
    let path = path.rsplit("::").next().unwrap_or(path);
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn cancelled(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "Cancelled"))
    } else {
        Ok(())
    }
}

impl Environments {
    pub fn for_difficulty(&self, characteristic: &str, difficulty: &str) -> Option<&str> {
        self.difficulties
            .get(&(characteristic.into(), difficulty.into()))
            .map(String::as_str)
            .or_else(|| self.fallback(characteristic))
    }

    fn fallback(&self, characteristic: &str) -> Option<&str> {
        if matches!(characteristic, "90Degree" | "360Degree") {
            self.all_directions.as_deref().or(self.default.as_deref())
        } else {
            self.default.as_deref()
        }
    }

    fn from_level(level: &Value, characteristic: impl Fn(&Value) -> Option<String>) -> Self {
        let mut result = Self {
            default: level.get("_environmentName").and_then(name),
            all_directions: level.get("_allDirectionsEnvironmentName").and_then(name),
            ..Self::default()
        };
        let overrides = level.get("_environmentNames").and_then(Value::as_array);
        if let Some(sets) = level
            .get("_previewDifficultyBeatmapSets")
            .and_then(Value::as_array)
        {
            for set in sets {
                let Some(characteristic) =
                    set.get("_beatmapCharacteristic").and_then(&characteristic)
                else {
                    continue;
                };
                let Some(entries) = set
                    .get("_previewDifficultyBeatmaps")
                    .and_then(Value::as_array)
                else {
                    continue;
                };
                for entry in entries {
                    let Some(difficulty) = entry
                        .get("_difficulty")
                        .and_then(Value::as_u64)
                        .and_then(|i| {
                            ["Easy", "Normal", "Hard", "Expert", "ExpertPlus"].get(i as usize)
                        })
                    else {
                        continue;
                    };
                    // Empty override lists and negative/out-of-range indices use the legacy default.
                    let environment = entry
                        .get("_environmentNameIdx")
                        .and_then(Value::as_u64)
                        .and_then(|i| overrides?.get(i as usize))
                        .and_then(name)
                        .or_else(|| result.fallback(&characteristic).map(str::to_owned));
                    if let Some(environment) = environment {
                        result
                            .difficulties
                            .insert((characteristic.clone(), (*difficulty).into()), environment);
                    }
                }
            }
        }
        result
    }
}

// BeatmapLevelsData contains gameplay data only. The BeatmapLevel ScriptableObject
// lives in aa/<platform>/*_pack_assets_all_*.bundle in the same installation.
pub fn load(
    bundle: &Path,
    level_id: &str,
    cancel: &AtomicBool,
) -> io::Result<Option<Environments>> {
    cancelled(cancel)?;
    let Some(streaming) = bundle
        .parent()
        .filter(|p| p.file_name().is_some_and(|n| n == "BeatmapLevelsData"))
        .and_then(Path::parent)
    else {
        return Ok(None);
    };
    let aa = streaming.join("aa");
    if !aa.is_dir() {
        return Ok(None);
    }
    let mut packs = Vec::new();
    let mut shared = Vec::new();
    for platform in std::fs::read_dir(aa)? {
        let platform = platform?.path();
        if !platform.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(platform)? {
            let path = entry?.path();
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            if filename.ends_with(".bundle") {
                if filename.contains("_pack_assets_all_") {
                    packs.push(path);
                } else if filename.starts_with("sharedassets_assets_all_") {
                    shared.push(path);
                }
            }
        }
    }
    packs.sort();
    shared.sort();
    for path in packs {
        cancelled(cancel)?;
        let Ok(collection) = AssetCollection::load_path(&path) else {
            continue;
        };
        for loaded in collection.serialized_files() {
            for (i, object) in loaded.file.objects.iter().enumerate() {
                cancelled(cancel)?;
                if object.class_id != 114 {
                    continue;
                }
                let limits = MonoBehaviourReadLimits::default();
                let Ok(header) = read_mono_behaviour(&loaded.file, i, limits) else {
                    continue;
                };
                if !header
                    .name
                    .eq_ignore_ascii_case(&format!("{level_id}BeatmapLevel"))
                {
                    continue;
                }
                let Ok(json) = read_mono_behaviour_json(&loaded.file, i, false, limits) else {
                    continue;
                };
                let Ok(level) = serde_json::from_str::<Value>(&json) else {
                    continue;
                };
                if !level
                    .get("_levelID")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id.eq_ignore_ascii_case(level_id))
                {
                    continue;
                }
                let mut characteristics = HashMap::new();
                // Scope references by serialized filename as well as path ID.
                collect_characteristics(&collection, &mut characteristics, cancel)?;
                for dependency in &shared {
                    cancelled(cancel)?;
                    if dependency.parent() != path.parent() {
                        continue;
                    }
                    if let Ok(collection) = AssetCollection::load_path(dependency) {
                        collect_characteristics(&collection, &mut characteristics, cancel)?;
                    }
                }
                let resolve = |reference: &Value| {
                    let file_id = reference.get("m_FileID")?.as_u64()?;
                    let path_id = reference.get("m_PathID")?.as_i64()?;
                    let file = reference_file(&loaded.file, &loaded.path, file_id)?;
                    characteristics.get(&(file, path_id)).cloned()
                };
                if let Some(sets) = level
                    .get("_previewDifficultyBeatmapSets")
                    .and_then(Value::as_array)
                {
                    if sets.iter().any(|set| {
                        set.get("_beatmapCharacteristic")
                            .and_then(resolve)
                            .is_none()
                    }) {
                        return Err(io::Error::new(io::ErrorKind::NotFound,
                            "music-pack characteristic references could not be resolved; keep the matching sharedassets bundle in aa/<platform>"));
                    }
                }
                return Ok(Some(Environments::from_level(&level, resolve)));
            }
        }
    }
    Ok(None)
}

fn reference_file(file: &SerializedFile, source: &str, file_id: u64) -> Option<String> {
    let path = if file_id == 0 {
        source
    } else {
        &file.externals.get((file_id - 1) as usize)?.path
    };
    Some(basename(path).to_ascii_lowercase())
}

fn collect_characteristics(
    collection: &AssetCollection,
    names: &mut HashMap<(String, i64), String>,
    cancel: &AtomicBool,
) -> io::Result<()> {
    for loaded in collection.serialized_files() {
        for (i, object) in loaded.file.objects.iter().enumerate() {
            cancelled(cancel)?;
            if object.class_id != 114 {
                continue;
            }
            let limits = MonoBehaviourReadLimits::default();
            let Ok(header) = read_mono_behaviour(&loaded.file, i, limits) else {
                continue;
            };
            if !header.name.ends_with("BeatmapCharacteristic") {
                continue;
            }
            let Ok(json) = read_mono_behaviour_json(&loaded.file, i, false, limits) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&json) else {
                continue;
            };
            if let Some(name) = value.get("_serializedName").and_then(Value::as_str) {
                names.insert(
                    (basename(&loaded.path).to_ascii_lowercase(), object.path_id),
                    name.into(),
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unity_archive_names_match_external_reference_names() {
        assert_eq!(basename("C:/game/pack.bundle::CAB-abc"), "CAB-abc");
        assert_eq!(basename("archive:/CAB-abc/CAB-abc"), "CAB-abc");
        assert_eq!(basename(r"C:\game\CAB-abc"), "CAB-abc");
    }

    #[test]
    fn standalone_bundle_has_no_companion_and_cancellation_is_respected() {
        assert!(load(Path::new("lfg"), "LFG", &AtomicBool::new(false))
            .unwrap()
            .is_none());
        assert_eq!(
            load(Path::new("lfg"), "LFG", &AtomicBool::new(true))
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::Interrupted
        );
    }

    #[test]
    fn preserves_per_difficulty_overrides_and_legacy_rotation_defaults() {
        let level = json!({
            "_environmentName": {"_environmentName": "NiceEnvironment"},
            "_allDirectionsEnvironmentName": {"_environmentName": "GlassDesertEnvironment"},
            "_environmentNames": [{"_environmentName": "PrismEnvironment"}, {"_environmentName": "WeaveEnvironment"}],
            "_previewDifficultyBeatmapSets": [{
                "_beatmapCharacteristic": "Standard",
                "_previewDifficultyBeatmaps": [
                    {"_difficulty": 0, "_environmentNameIdx": 0},
                    {"_difficulty": 4, "_environmentNameIdx": 1},
                    {"_difficulty": 2, "_environmentNameIdx": -1}
                ]
            }]
        });
        let env = Environments::from_level(&level, |v| v.as_str().map(str::to_owned));
        assert_eq!(
            env.for_difficulty("Standard", "Easy"),
            Some("PrismEnvironment")
        );
        assert_eq!(
            env.for_difficulty("Standard", "ExpertPlus"),
            Some("WeaveEnvironment")
        );
        assert_eq!(
            env.for_difficulty("Standard", "Hard"),
            Some("NiceEnvironment")
        );
        assert_eq!(
            env.for_difficulty("360Degree", "Easy"),
            Some("GlassDesertEnvironment")
        );
    }

    #[test]
    fn empty_or_invalid_override_slots_do_not_shift_indices() {
        let level = json!({
            "_environmentName": "DefaultEnvironment",
            "_environmentNames": [null, {"_environmentName": "WeaveEnvironment"}],
            "_previewDifficultyBeatmapSets": [{
                "_beatmapCharacteristic": "Standard",
                "_previewDifficultyBeatmaps": [{"_difficulty": 0, "_environmentNameIdx": 1}]
            }]
        });
        let env = Environments::from_level(&level, |v| v.as_str().map(str::to_owned));
        assert_eq!(
            env.for_difficulty("Standard", "Easy"),
            Some("WeaveEnvironment")
        );
        let env = Environments::from_level(
            &json!({"_environmentName": {"_environmentName": "NiceEnvironment"}, "_environmentNames": []}),
            |_| None,
        );
        assert_eq!(
            env.for_difficulty("Standard", "Easy"),
            Some("NiceEnvironment")
        );
    }
}
