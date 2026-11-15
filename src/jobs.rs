use crate::map_info::{self, ExtractedAsset};
use flate2::read::MultiGzDecoder;
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
};
use unity_rs_core::{
    export::{export_collection_with_plan, ExportMode, ExportOptions, ExportPlan, ExportRecord},
    loader::AssetCollection,
};

const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024 * 1024;

pub struct Destination {
    pub folder_name: String,
    pub overwrite: bool,
}

pub fn bundle_folder_name(input: &Path) -> String {
    safe_name(&input.file_name().unwrap_or_default().to_string_lossy())
}

pub fn validate_folder_name(name: &str) -> io::Result<()> {
    if name.is_empty() || name.trim() != name || safe_name(name) != name {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Enter a folder name without path separators, reserved names, or trailing spaces.",
        ))
    } else {
        Ok(())
    }
}

pub fn destination_exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

pub enum Event {
    Log(String),
    Progress(f32, String),
    Finished {
        status: String,
        output: Option<PathBuf>,
    },
}

fn cancelled(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "Cancelled"))
    } else {
        Ok(())
    }
}

fn progress(tx: &Sender<Event>, fraction: f32, message: &str) {
    let _ = tx.send(Event::Progress(fraction, message.into()));
    let _ = tx.send(Event::Log(message.into()));
}

// Filenames from bundles must be a single portable component on either OS.
fn safe_name(name: &str) -> String {
    let mut result: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    if result.len() > 190 {
        let mut end = 190;
        while !result.is_char_boundary(end) {
            end -= 1;
        }
        result.truncate(end);
    }
    result = result.trim_matches(|c| c == '.' || c == ' ').to_owned();
    if result.is_empty() {
        result = "asset".into();
    }
    let stem = result.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit())
    {
        result.insert(0, '_');
    }
    result
}

fn unique_name(name: &str, used: &mut HashSet<String>) -> String {
    let name = safe_name(name);
    let path = Path::new(&name);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    let mut candidate = name.clone();
    let mut index = 2;
    while !used.insert(candidate.to_lowercase()) {
        candidate = format!("{stem}-{index}{extension}");
        index += 1;
    }
    candidate
}

// Read with cancellation and expansion limits; never leave an incomplete destination.
fn save_stream(
    mut reader: impl Read,
    destination: &Path,
    cancel: &AtomicBool,
    limit: u64,
) -> io::Result<u64> {
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::other("Missing output parent"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut buffer = [0_u8; 128 * 1024];
    let mut written = 0_u64;
    loop {
        cancelled(cancel)?;
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        written += count as u64;
        if written > limit {
            return Err(io::Error::other(
                "Extracted data exceeds the output size limit",
            ));
        }
        temporary.write_all(&buffer[..count])?;
    }
    temporary.as_file_mut().sync_all()?;
    cancelled(cancel)?;
    temporary.persist(destination).map_err(|err| err.error)?;
    Ok(written)
}

fn dat_name(name: &str) -> String {
    let mut base = name.to_owned();
    for suffix in [".asset", ".txt", ".bytes"] {
        if base.to_ascii_lowercase().ends_with(suffix) {
            base.truncate(base.len() - suffix.len());
            break;
        }
    }
    if base.to_ascii_lowercase().ends_with(".gz") {
        base.truncate(base.len() - 3);
    }
    if !base.to_ascii_lowercase().ends_with(".dat") {
        base.push_str(".dat");
    }
    base
}

fn prepare_asset(
    record: &ExportRecord,
    folder: &Path,
    used: &mut HashSet<String>,
    cancel: &AtomicBool,
    remaining: u64,
) -> io::Result<(String, u64)> {
    let mut source = BufReader::new(File::open(&record.output_path)?);
    let mut signature = [0; 2];
    let count = source.read(&mut signature)?;
    drop(source);
    let source = BufReader::new(File::open(&record.output_path)?);
    let name = record
        .output_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let gzip = record.class_id == 49
        && record.payload_kind == "text_bytes"
        && count == 2
        && signature == [0x1f, 0x8b];
    let suggested_name = if record.payload_kind == "raw" {
        let mut path = PathBuf::from(name.as_ref());
        path.set_extension("bin");
        path.to_string_lossy().into_owned()
    } else if gzip {
        dat_name(&name)
    } else {
        name.into_owned()
    };
    let name = unique_name(&suggested_name, used);
    let path = folder.join(&name);
    let limit = remaining.min(MAX_FILE_BYTES);
    let size = if gzip {
        save_stream(MultiGzDecoder::new(source), &path, cancel, limit)?
    } else {
        save_stream(source, &path, cancel, limit)?
    };
    Ok((name, size))
}

pub fn run(
    input: PathBuf,
    output: PathBuf,
    destination: Destination,
    tx: Sender<Event>,
    cancel: Arc<AtomicBool>,
) {
    // Keep decoder panics from leaving the UI permanently busy.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        extract(&input, &output, &destination, &tx, &cancel)
    }));
    let (status, output) = match result {
        Ok(Ok((status, output))) => (status, Some(output)),
        Ok(Err(err)) if err.kind() == io::ErrorKind::Interrupted => (
            "Cancelled. No incomplete extraction was saved.".into(),
            None,
        ),
        Ok(Err(err)) => (format!("Extraction failed: {err}"), None),
        Err(_) => (
            "The asset decoder stopped unexpectedly. See Console.".into(),
            None,
        ),
    };
    let _ = tx.send(Event::Finished { status, output });
}

fn extract(
    input: &Path,
    output: &Path,
    target: &Destination,
    tx: &Sender<Event>,
    cancel: &AtomicBool,
) -> io::Result<(String, PathBuf)> {
    cancelled(cancel)?;
    if !input.is_file() {
        return Err(io::Error::other("Select an existing bundle file"));
    }
    fs::create_dir_all(output)?;
    let output = fs::canonicalize(output)?;
    validate_folder_name(&target.folder_name)?;
    let destination = output.join(&target.folder_name);
    if target.overwrite && destination_exists(&destination)? {
        let metadata = fs::symlink_metadata(&destination)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io::Error::other(
                "Only an existing directory can be overwritten; choose a new folder name.",
            ));
        }
        if fs::canonicalize(input)?.starts_with(fs::canonicalize(&destination)?) {
            return Err(io::Error::other(
                "The input bundle is inside the destination; choose a new folder to preserve it.",
            ));
        }
    } else if destination_exists(&destination)? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "The destination already exists. Extract again to choose a new name or overwrite.",
        ));
    }
    let staging = tempfile::Builder::new()
        .prefix(".asset-extractor-")
        .tempdir_in(&output)?;
    let ready = staging.path().join("ready");
    fs::create_dir(&ready)?;
    progress(tx, 0.02, "Opening and decompressing bundle...");
    let collection = AssetCollection::load_path(input).map_err(io::Error::other)?;
    cancelled(cancel)?;
    for diagnostic in &collection.diagnostics {
        let _ = tx.send(Event::Log(format!(
            "Load warning ({}): {}",
            diagnostic.path, diagnostic.message
        )));
    }
    let count: usize = collection
        .serialized_files()
        .iter()
        .map(|loaded| loaded.file.objects.len())
        .sum();
    if count == 0 {
        return Err(io::Error::other(
            "No Unity objects were found in this bundle",
        ));
    }
    let _ = tx.send(Event::Log(format!("Found {count} Unity objects.")));
    let mut other_classes: Vec<i32> = collection
        .serialized_files()
        .iter()
        .flat_map(|loaded| loaded.file.objects.iter().map(|object| object.class_id))
        .filter(|id| ![49, 28, 83].contains(id))
        .collect();
    other_classes.sort_unstable();
    other_classes.dedup();
    let stages: [(&str, &[i32]); 4] = [
        ("Extracting map data...", &[49]),
        ("Exporting textures...", &[28]),
        ("Exporting audio...", &[83]),
        ("Exporting other assets...", &other_classes),
    ];
    let mut used = HashSet::new();
    // Preserve a bundled Info.dat under a numbered name; generate the final entry point last.
    used.insert("info.dat".into());
    // Reserve the editor's required timing filename before assigning exported asset names.
    used.insert(map_info::AUDIO_DATA_FILENAME.to_lowercase());
    let mut extracted_assets = Vec::new();
    let (mut exported, mut warnings, mut total_bytes) =
        (0_usize, collection.diagnostics.len(), 0_u64);
    for (stage, (label, classes)) in stages.iter().enumerate() {
        cancelled(cancel)?;
        progress(tx, 0.1 + stage as f32 * 0.2, label);
        if classes.is_empty() {
            continue;
        }
        let stage_output = staging.path().join(format!("stage-{stage}"));
        let report = export_collection_with_plan(
            &collection,
            &stage_output,
            ExportOptions {
                maximum_total_output_bytes: MAX_TOTAL_BYTES.saturating_sub(total_bytes),
                ..ExportOptions::default()
            },
            ExportPlan {
                classes: Some(classes),
                ..ExportPlan::default()
            },
        )
        .map_err(io::Error::other)?;
        cancelled(cancel)?;
        for failure in report.failures.iter().chain(report.unsupported.iter()) {
            warnings += 1;
            let _ = tx.send(Event::Log(format!(
                "Warning: class {}, object {}: {}",
                failure.class_id, failure.path_id, failure.error
            )));
        }
        let mut records = report.exported;
        // Keep unsupported objects as raw Unity bytes as well as reporting their limitations.
        let mut fallback_classes: Vec<i32> = report
            .failures
            .iter()
            .chain(report.unsupported.iter())
            .map(|f| f.class_id)
            .collect();
        fallback_classes.sort_unstable();
        fallback_classes.dedup();
        if !fallback_classes.is_empty() {
            cancelled(cancel)?;
            let fallback = export_collection_with_plan(
                &collection,
                &staging.path().join(format!("raw-{stage}")),
                ExportOptions {
                    mode: ExportMode::Raw,
                    maximum_total_output_bytes: MAX_TOTAL_BYTES.saturating_sub(total_bytes),
                    ..ExportOptions::default()
                },
                ExportPlan {
                    classes: Some(&fallback_classes),
                    ..ExportPlan::default()
                },
            )
            .map_err(io::Error::other)?;
            let successful: HashSet<_> = records
                .iter()
                .map(|r| (r.source.clone(), r.path_id))
                .collect();
            records.extend(
                fallback
                    .exported
                    .into_iter()
                    .filter(|r| !successful.contains(&(r.source.clone(), r.path_id))),
            );
            for failure in fallback.failures.iter().chain(fallback.unsupported.iter()) {
                warnings += 1;
                let _ = tx.send(Event::Log(format!(
                    "Raw fallback failed for object {}: {}",
                    failure.path_id, failure.error
                )));
            }
        }
        for record in records {
            cancelled(cancel)?;
            match prepare_asset(
                &record,
                &ready,
                &mut used,
                cancel,
                MAX_TOTAL_BYTES.saturating_sub(total_bytes),
            ) {
                Ok((name, bytes)) => {
                    exported += 1;
                    total_bytes += bytes;
                    extracted_assets.push(ExtractedAsset {
                        filename: name.clone(),
                        original_filename: record
                            .output_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        class_id: record.class_id,
                        payload_kind: record.payload_kind,
                    });
                    let _ = tx.send(Event::Log(format!("Saved: {name} ({bytes} bytes)")));
                }
                Err(err) if err.kind() == io::ErrorKind::Interrupted => return Err(err),
                Err(err) => {
                    warnings += 1;
                    let _ = tx.send(Event::Log(format!(
                        "Could not prepare {}: {err}",
                        record.output_path.display()
                    )));
                }
            }
        }
        fs::remove_dir_all(stage_output)?;
        let raw_output = staging.path().join(format!("raw-{stage}"));
        if raw_output.exists() {
            fs::remove_dir_all(raw_output)?;
        }
    }
    cancelled(cancel)?;
    if exported == 0 {
        return Err(io::Error::other(
            "No assets could be exported. See Console for details.",
        ));
    }
    progress(tx, 0.92, "Generating Info.dat...");
    let info = map_info::generate(&ready, input, &extracted_assets, cancel)?;
    for warning in info.warnings {
        warnings += 1;
        let _ = tx.send(Event::Log(format!("Warning: {warning}")));
    }
    if let Some(source) = &info.audio_data_source {
        cancelled(cancel)?;
        // Rename only: preserve the complete timing payload, BPM regions, and checksum.
        fs::rename(
            ready.join(source),
            ready.join(map_info::AUDIO_DATA_FILENAME),
        )?;
        let _ = tx.send(Event::Log(format!(
            "Audio timing: {source} -> {} (editor-compatible filename)",
            map_info::AUDIO_DATA_FILENAME
        )));
    }
    save_stream(
        info.bytes.as_slice(),
        &ready.join("Info.dat"),
        cancel,
        MAX_TOTAL_BYTES
            .saturating_sub(total_bytes)
            .min(MAX_FILE_BYTES),
    )?;
    let _ = tx.send(Event::Log("Generated: Info.dat (v4.0.1)".into()));
    progress(tx, 0.95, "Saving extracted assets...");
    cancelled(cancel)?;
    if let Some(warning) = publish(&ready, &destination, target.overwrite, tx)? {
        warnings += 1;
        let _ = tx.send(Event::Log(format!("Warning: {warning}")));
    }
    let status = format!("Done: {exported} assets extracted, {warnings} warnings");
    progress(tx, 1.0, &status);
    let _ = tx.send(Event::Log(format!("Saved to: {}", destination.display())));
    Ok((status, destination))
}

// The previous directory is moved aside only after decoding and preparation succeed.
// Its backup is persisted before moving data so a panic cannot drop the old extraction.
fn publish(
    ready: &Path,
    destination: &Path,
    overwrite: bool,
    tx: &Sender<Event>,
) -> io::Result<Option<String>> {
    let backup = if destination_exists(destination)? {
        if !overwrite {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists,
                "The destination appeared during extraction. Extract again to choose how to handle it."));
        }
        let metadata = fs::symlink_metadata(destination)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io::Error::other(
                "The destination is no longer a regular directory; it was not overwritten.",
            ));
        }
        let backup = tempfile::Builder::new()
            .prefix(".asset-extractor-backup-")
            .tempdir_in(
                destination
                    .parent()
                    .ok_or_else(|| io::Error::other("Missing destination parent"))?,
            )?
            .keep();
        if let Err(err) = fs::rename(destination, backup.join("previous")) {
            let _ = fs::remove_dir(&backup);
            return Err(err);
        }
        let _ = tx.send(Event::Log(format!(
            "Previous extraction backed up at: {}",
            backup.join("previous").display()
        )));
        Some(backup)
    } else {
        None
    };
    let mut created = false;
    let result = (|| -> io::Result<()> {
        fs::create_dir(destination)?;
        created = true;
        for entry in fs::read_dir(ready)? {
            let entry = entry?;
            fs::rename(entry.path(), destination.join(entry.file_name()))?;
        }
        Ok(())
    })();
    if let Err(err) = result {
        if created {
            if let Err(cleanup) = fs::remove_dir_all(destination) {
                return Err(io::Error::other(format!(
                    "Save failed: {err}. Could not remove incomplete output {}: {cleanup}. Previous extraction backup: {}",
                    destination.display(), backup.as_ref().map(|p| p.join("previous").display().to_string()).unwrap_or_else(|| "none".into())
                )));
            }
        }
        if let Some(backup) = &backup {
            if let Err(restore) = fs::rename(backup.join("previous"), destination) {
                return Err(io::Error::other(format!(
                    "Save failed: {err}. Could not restore the previous extraction: {restore}. It is preserved at {}",
                    backup.join("previous").display()
                )));
            }
            let _ = fs::remove_dir(backup);
        }
        return Err(err);
    }
    if let Some(backup) = backup {
        if let Err(err) = fs::remove_dir_all(&backup) {
            return Ok(Some(format!(
                "Extraction saved, but the previous backup could not be removed: {} ({err})",
                backup.display()
            )));
        }
    }
    Ok(None)
}
