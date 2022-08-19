use anyhow::{Context, Result};
use clap::Parser;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[clap(author, version)]
/// Combine exported harmony datafiles into a single TSV file
struct Args {
    /// directory to search for harmony files
    #[clap(value_parser)]
    input: PathBuf,
    /// output file name, or stdout if not present
    #[clap(value_parser)]
    output: Option<PathBuf>,
    /// Create separate output files for each population
    #[clap(short, long, action, requires = "output")]
    separate: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    match (args.separate, args.output.as_deref()) {
        (true, Some(out)) => separate_by_pop(&args.input, out),
        _ => combine_files(&args.input, args.output.as_deref()),
    }
}

fn combine_files(dir: &Path, out: Option<&Path>) -> Result<()> {
    let mut stdout;
    let mut fbuf;

    let wtr = if let Some(p) = out {
        fbuf = create_bufwriter(p)?;
        &mut fbuf as &mut dyn Write
    } else {
        stdout = std::io::stdout().lock();
        &mut stdout as &mut dyn Write
    };

    let metadata = harmony::collect_harmony_datafiles(dir);
    if metadata.is_empty() {
        anyhow::bail!("did not find any harmony files");
    }
    harmony::combine_files(wtr, &metadata).context("combining files")?;
    Ok(())
}

fn separate_by_pop(dir: &Path, out: &Path) -> Result<()> {
    let pops = harmony::iterate_harmony_datafiles(dir).fold(HashMap::new(), |mut map, md| {
        let pop = md.population.clone();
        map.entry(pop).or_insert_with(Vec::new).push(md);

        map
    });

    // check that atleast one file was found
    if pops.iter().all(|(_, v)| v.is_empty()) {
        anyhow::bail!("did not find any harmony files");
    }

    let basename = out
        .file_stem()
        .map(|os| os.to_string_lossy())
        .ok_or_else(|| anyhow::anyhow!("No file stem for output <{}?", out.display()))?;

    let iter = pops
        .iter()
        .map(|(k, v)| (k.as_deref().unwrap_or("WellData"), v));
    for (pop, metadata) in iter {
        let p = out.with_file_name(format!("{}_{}.tsv", basename, pop));
        let wtr = create_bufwriter(p)?;
        harmony::combine_files(wtr, &metadata)
            .with_context(|| format!("combining population: {}", pop))?;
    }

    Ok(())
}

fn create_bufwriter<P: AsRef<Path>>(p: P) -> Result<BufWriter<File>> {
    let p = p.as_ref();
    File::create(p)
        .map(BufWriter::new)
        .with_context(|| format!("creating output file {}", p.display()))
}
