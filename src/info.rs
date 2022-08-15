use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use crate::utils::{read_lines, StrIntern};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone)]
pub struct HarmonyMetadata {
    pub path: PathBuf,
    pub db_name: Rc<str>,
    pub db_location: Rc<str>,
    pub eval_sig: String,
    pub plate_name: String,
    pub measurement: u32,
    pub evaluation: u32,
    pub population: Option<Rc<str>>,
    pub headers: Vec<Rc<str>>,
    /// line of first data row
    pub data_start: u8,
}

pub fn iterate_harmony_datafiles<P: AsRef<Path>>(dir: P) -> impl Iterator<Item = HarmonyMetadata> {
    let mut interner = StrIntern::new();

    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok().filter(is_possible_harmony))
        .filter_map(move |e| {
            read_harmony_metadata(&e, &mut interner).and_then(|m| m.finalize(e.into_path()))
        })
}

pub fn collect_harmony_datafiles<P: AsRef<Path>>(dir: P) -> Vec<HarmonyMetadata> {
    iterate_harmony_datafiles(dir).collect()
}

fn is_possible_harmony(f: &DirEntry) -> bool {
    f.file_type().is_file()
        && f.path().extension().map_or(false, |ext| ext == "txt")
        && f.path().file_stem().map_or(false, |stem| {
            stem == "PlateResults" || stem.to_string_lossy().starts_with("Objects_Population")
        })
}

#[derive(Debug, Default)]
struct CollectMetadata {
    db_name: Option<Rc<str>>,
    db_location: Option<Rc<str>>,
    eval_sig: Option<String>,
    plate_name: Option<String>,
    measurement: Option<u32>,
    evaluation: Option<u32>,
    population: Option<Rc<str>>,
    headers: Option<Vec<Rc<str>>>,
    data_start: Option<u8>,
}

impl CollectMetadata {
    fn finalize(self, path: PathBuf) -> Option<HarmonyMetadata> {
        let s = self;
        match (
            s.db_name,
            s.db_location,
            s.eval_sig,
            s.plate_name,
            s.measurement,
            s.evaluation,
            s.headers,
            s.data_start,
        ) {
            (
                Some(db_name),
                Some(db_location),
                Some(eval_sig),
                Some(plate_name),
                Some(measurement),
                Some(evaluation),
                Some(headers),
                Some(data_start),
            ) => Some(HarmonyMetadata {
                path,
                db_name,
                db_location,
                eval_sig,
                plate_name,
                measurement,
                evaluation,
                population: s.population,
                headers,
                data_start,
            }),
            _ => None,
        }
    }
}

fn read_harmony_metadata(f: &DirEntry, interner: &mut StrIntern) -> Option<CollectMetadata> {
    let mut output = CollectMetadata::default();
    let mut into_data = false;

    for (i, res) in read_lines(f.path()).ok()?.enumerate() {
        let line = res.ok()?;
        let line = line.trim();

        // after a [Data] line, read until the headers
        match line {
            "[Data]" => into_data = true,
            "" => continue,
            _ => {
                if !into_data {
                    process_metadata_kv(line, interner, &mut output)?;
                } else {
                    // collect header row
                    let hdrs = line.split('\t').map(|col| interner.get(col)).collect();
                    output.headers = Some(hdrs);
                    // the data starts on the next row
                    output.data_start = Some(i as u8 + 1);
                    break;
                }
            }
        }
    }
    Some(output)
}

fn process_metadata_kv(
    line: &str,
    interner: &mut StrIntern,
    store: &mut CollectMetadata,
) -> Option<()> {
    let mut parts = line.split('\t');
    let key = parts.next()?;
    let value = parts.next()?;
    // store the values into the temp struct
    match key {
        "Database Name" => {
            store.db_name = Some(interner.get(value));
        }
        "Database Location" | "Database Link" => {
            store.db_location = Some(interner.get(value));
        }
        "Evaluation Signature" => store.eval_sig = Some(value.into()),
        "Plate Name" => store.plate_name = Some(value.into()),
        "Measurement" => store.measurement = value.split(" ").nth(1).and_then(|s| s.parse().ok()),
        "Evaluation" => store.evaluation = value[10..].parse().ok(),
        "Population" => store.population = Some(value.into()),
        _ => (),
    }

    Some(())
}
