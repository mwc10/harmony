use std::{
    collections::HashSet,
    fs::File,
    io::{self, BufRead, BufReader, Lines},
    path::{Path, PathBuf},
    rc::Rc,
};

use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone)]
pub struct HarmonyMetadata {
    pub db_name: Rc<str>,
    pub db_location: Rc<str>,
    pub eval_sig: String,
    pub plate_name: String,
    pub measurement: u32,
    pub evaluation: u32,
    pub population: Option<Rc<str>>,
}

pub fn find_harmony_datafiles<P: AsRef<Path>>(dir: P) -> Vec<(PathBuf, HarmonyMetadata)> {
    let mut interner = StrIntern::new();

    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok().filter(is_possible_harmony))
        .filter_map(|e| read_harmony_metadata(&e, &mut interner).map(|m| (e.into_path(), m)))
        .collect()
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
}

impl CollectMetadata {
    fn convert(self) -> Option<HarmonyMetadata> {
        let s = self;
        match (
            s.db_name,
            s.db_location,
            s.eval_sig,
            s.plate_name,
            s.measurement,
            s.evaluation,
        ) {
            (
                Some(db_name),
                Some(db_location),
                Some(eval_sig),
                Some(plate_name),
                Some(measurement),
                Some(evaluation),
            ) => Some(HarmonyMetadata {
                db_name,
                db_location,
                eval_sig,
                plate_name,
                measurement,
                evaluation,
                population: s.population,
            }),
            _ => None,
        }
    }
}

fn read_harmony_metadata(f: &DirEntry, interner: &mut StrIntern) -> Option<HarmonyMetadata> {
    let mut output = CollectMetadata::default();

    for res in read_lines(f.path()).ok()? {
        let line = res.ok()?;
        let line = line.trim();

        // stop reading at the data line
        match line {
            "[Data]" => break,
            "" => continue,
            _ => (),
        }
        let mut parts = line.split('\t');
        let key = parts.next()?;
        let value = parts.next()?;
        // store the values into the temp struct
        match key {
            "Database Name" => {
                let val = interner.get(value);
                output.db_name = Some(val);
            }
            "Database Location" | "Database Link" => {
                let val = interner.get(value);
                output.db_location = Some(val);
            }
            "Evaluation Signature" => output.eval_sig = Some(value.into()),
            "Plate Name" => output.plate_name = Some(value.into()),
            "Measurement" => {
                output.measurement = value.split(" ").nth(1).and_then(|s| s.parse().ok())
            }
            "Evaluation" => output.evaluation = value[10..].parse().ok(),
            "Population" => output.population = Some(value.into()),
            _ => (),
        }
    }
    output.convert()
}

fn read_lines<P: AsRef<Path>>(p: P) -> io::Result<Lines<BufReader<File>>> {
    File::open(p).map(|f| BufReader::new(f).lines())
}

struct StrIntern(HashSet<Rc<str>>);

impl StrIntern {
    fn new() -> Self {
        Self(HashSet::new())
    }
    fn get(&mut self, s: &str) -> Rc<str> {
        if let Some(interned) = self.0.get(s) {
            Rc::clone(interned)
        } else {
            let interned = Rc::from(s);
            self.0.insert(Rc::clone(&interned));
            interned
        }
    }
}
