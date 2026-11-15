use flate2::read::MultiGzDecoder;
use std::{
    fs::{self, File},
    io::{self, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
    time::{Duration, Instant},
};
use tempfile::NamedTempFile;

pub enum Job {
    Convert { input: PathBuf, output: PathBuf },
    Extract { input: PathBuf, output: PathBuf },
}

pub enum Event {
    Log(String),
    Progress(f32, String),
    Finished(String),
}

fn cancelled(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "Cancelled"))
    } else {
        Ok(())
    }
}

// Commit only complete files. A failed or cancelled operation leaves any previous output intact.
fn save_stream(
    mut reader: impl Read,
    destination: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> io::Result<()> {
    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "Output has no parent folder")
    })?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    let mut buffer = [0_u8; 128 * 1024];
    let mut written = 0_u64;
    loop {
        cancelled(cancel)?;
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        temporary.write_all(&buffer[..count])?;
        written += count as u64;
        progress(written);
    }
    cancelled(cancel)?;
    temporary.as_file_mut().sync_all()?;
    cancelled(cancel)?;
    temporary.persist(destination).map_err(|err| err.error)?;
    Ok(())
}

pub fn run(job: Job, tx: &Sender<Event>, cancel: Arc<AtomicBool>) {
    let result = match job {
        Job::Convert { input, output } => convert(&input, &output, tx, &cancel),
        Job::Extract { input, output } => extract(&input, &output, tx, &cancel),
    };
    let status = match result {
        Ok(status) => status,
        Err(err) if err.kind() == io::ErrorKind::Interrupted => "Cancelled".into(),
        Err(err) => format!("Failed: {err}"),
    };
    let _ = tx.send(Event::Finished(status));
}

fn convert(
    input: &Path,
    output: &Path,
    tx: &Sender<Event>,
    cancel: &AtomicBool,
) -> io::Result<String> {
    if !input.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Input folder does not exist",
        ));
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(input)? {
        cancelled(cancel)?;
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.file_name().to_string_lossy().ends_with(".gz.asset")
        {
            files.push(entry.path());
        }
    }
    files.sort();
    if files.is_empty() {
        return Ok("No .gz.asset files found in the input folder".into());
    }
    fs::create_dir_all(output)?;
    let total = files.len();
    let (mut converted, mut errors) = (0, 0);
    for (index, file) in files.iter().enumerate() {
        cancelled(cancel)?;
        let name = file.file_name().unwrap().to_string_lossy();
        let destination = output.join(format!("{}.dat", name.strip_suffix(".gz.asset").unwrap()));
        let result = File::open(file).and_then(|source| {
            save_stream(
                MultiGzDecoder::new(BufReader::new(source)),
                &destination,
                cancel,
                |_| {},
            )
        });
        match result {
            Ok(()) => {
                converted += 1;
                let _ = tx.send(Event::Log(format!(
                    "Converted: {name} -> {}",
                    destination.file_name().unwrap().to_string_lossy()
                )));
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {
                return Ok(format!("Cancelled: {converted} converted, {errors} errors"));
            }
            Err(err) => {
                errors += 1;
                let _ = tx.send(Event::Log(format!("Error processing {name}: {err}")));
            }
        }
        let _ = tx.send(Event::Progress(
            (index + 1) as f32 / total as f32,
            format!("{}/{total} processed", index + 1),
        ));
    }
    Ok(format!("Done: {converted} converted, {errors} errors"))
}

fn extract(
    input: &Path,
    output: &Path,
    tx: &Sender<Event>,
    cancel: &AtomicBool,
) -> io::Result<String> {
    let mut source = File::open(input)?;
    let size = source.metadata()?.len();
    if !source.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Select a bundle file",
        ));
    }
    fs::create_dir_all(output)?;
    let destination = output.join("raw_asset_bundle.unity3d");
    if destination.exists() && fs::canonicalize(input)? == fs::canonicalize(&destination)? {
        cancelled(cancel)?;
        let _ = tx.send(Event::Progress(
            1.0,
            "Bundle already in output folder".into(),
        ));
        return Ok("Done: raw bundle already in output folder".into());
    }
    let mut last_update = Instant::now();
    save_stream(&mut source, &destination, cancel, |written| {
        if last_update.elapsed() >= Duration::from_millis(100) {
            let fraction = if size == 0 {
                1.0
            } else {
                written as f32 / size as f32
            };
            let _ = tx.send(Event::Progress(
                fraction,
                format!("Copying: {written} / {size} bytes"),
            ));
            last_update = Instant::now();
        }
    })?;
    let _ = tx.send(Event::Log(format!(
        "Raw bundle saved: {}",
        destination.display()
    )));
    let _ = tx.send(Event::Progress(1.0, "Raw bundle saved".into()));
    Ok("Done: raw bundle ready for AssetStudio".into())
}
