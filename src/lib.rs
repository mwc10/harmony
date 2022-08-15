mod info;
mod utils;

use std::{
    collections::HashSet,
    fmt::Display,
    fs::File,
    io::{self, BufRead, BufReader, Write},
    ops::Deref,
    path::Path,
    rc::Rc,
};

pub use info::{collect_harmony_datafiles, iterate_harmony_datafiles, HarmonyMetadata};

pub fn combine_files(metadata: &[HarmonyMetadata]) {
    let test = combine_headers(metadata);
    // println!("{:#?}", test);
    let mut stdout = io::stdout().lock();
    write_output(&mut stdout, metadata, test).expect("no io errors plz");
}

// combine all headers to find all columns
//  -> if same quick path
//  -> else create map (as simple array) between a idx of file's header to idx of combined header
// add metadata fields to header
// create output file and write header
// loop thru each file and copy data?

fn combine_headers(metadata: &[HarmonyMetadata]) -> Option<Combined> {
    let mut iter = metadata
        .iter()
        .map(|m| m.headers.iter().map(Rc::clone).collect::<HashSet<_>>());
    if let Some(base) = iter.next() {
        let differences: HashSet<Rc<str>> = iter.fold(HashSet::new(), |mut diffs, other| {
            diffs.extend(base.symmetric_difference(&other).cloned());
            diffs
        });

        if differences.is_empty() {
            return None;
        }
        // creating canonical header
        // find the columns in differences that are not in base
        // this gets rid of any columns that were in the base file, but were not in a later file
        let new_hdrs = {
            let mut n = differences.difference(&base).cloned().collect::<Vec<_>>();
            n.sort();
            metadata[0]
                .headers
                .iter()
                .cloned()
                .chain(n)
                .collect::<Vec<_>>()
        };
        // create map between original file headers and the combined new headers
        let maps = metadata
            .iter()
            .map(|m| {
                m.headers
                    .iter()
                    .map(
                        |orig| match new_hdrs.iter().position(|s| orig.deref() == s.deref()) {
                            Some(idx) => idx,
                            None => {
                                eprintln!("missing\t<{}>\t in:", orig);
                                eprintln!("{:#?}", &new_hdrs);
                                panic!("new combined headers should contain all headers");
                            }
                        },
                    )
                    .collect()
            })
            .collect();

        Some(Combined {
            hdr: new_hdrs,
            maps,
        })
    } else {
        None
    }
}

#[derive(Debug)]
struct Combined {
    hdr: Vec<Rc<str>>,
    maps: Vec<Vec<usize>>,
}

fn write_output(
    wtr: &mut impl Write,
    md: &[HarmonyMetadata],
    combination: Option<Combined>,
) -> io::Result<()> {
    let mut buf = String::with_capacity(0x400);
    // write headers for output fil
    // common fields then data headers
    write_interspersed(wtr, COMMON_FIELD_HDR, "\t")?;
    write!(wtr, "\t")?;
    let hdr = combination
        .as_ref()
        .map(|c| c.hdr.as_slice())
        .or(md.get(0).map(|f| f.headers.as_slice()))
        .expect("at least one header");
    write_interspersed(wtr, hdr, "\t")?;
    writeln!(wtr)?;

    if let Some(comb) = combination {
        // open file
        for (md, map) in md.iter().zip(comb.maps.iter()) {
            // generate common field
            let common_info = generate_common_fields(md, "\t");

            // open file and skip ahead to data
            let mut rdr = open_bufread(&md.path)?;
            let mut line = 0;
            while line < md.data_start && rdr.read_line(&mut buf).is_ok() {
                buf.clear();
                line += 1;
            }
            // read each line, then map the data into the output order, then write
            buf.clear();
            while rdr.read_line(&mut buf)? != 0 {
                // todo: can this be moved somewhere else to avoid lifetime issues?
                let mut output_line = vec![""; comb.hdr.len()];
                for (data, outidx) in buf.split('\t').zip(map.iter().copied()) {
                    output_line[outidx] = data;
                }
                write!(wtr, "{common_info}\t")?;
                write_interspersed(wtr, &output_line, "\t")?;
                writeln!(wtr)?;
                buf.clear();
            }
        }
    } else {
        for md in md {
            // generate common field
            let common_info = generate_common_fields(md, "\t");
            // open file and skip ahead to data
            let mut rdr = open_bufread(&md.path)?;
            let mut line = 0;
            while line < md.data_start && rdr.read_line(&mut buf).is_ok() {
                buf.clear();
                line += 1;
            }
            // read each line, then output common fields + data fields
            buf.clear();
            while rdr.read_line(&mut buf)? != 0 {
                let line = buf.trim();
                writeln!(wtr, "{common_info}\t{line}")?;
                buf.clear();
            }
        }
    }

    Ok(())
}

fn write_interspersed(w: &mut impl Write, items: &[impl Display], sep: &str) -> io::Result<()> {
    let mut need_sep = false;

    for item in items {
        if need_sep {
            write!(w, "{sep}")?;
        } else {
            need_sep = true;
        }
        write!(w, "{item}")?;
    }

    Ok(())
}

fn open_bufread(p: &Path) -> io::Result<BufReader<File>> {
    File::open(p).map(BufReader::new)
}

const COMMON_FIELD_HDR: &[&str] = &[
    "Plate Name",
    "Measurement",
    "Evaluation",
    "Evaluation Signature",
    "Population",
];

fn generate_common_fields(md: &HarmonyMetadata, sep: &str) -> String {
    let measurement = format!("{}", md.measurement);
    let eval = format!("{}", md.evaluation);
    let common_info = &[
        md.plate_name.as_str(),
        measurement.as_str(),
        eval.as_str(),
        md.eval_sig.as_str(),
        md.population.as_deref().unwrap_or("Well"),
    ];

    let mut out = Vec::with_capacity(0x100);
    write_interspersed(&mut out, common_info, sep).expect("no io issues with memory writes");

    String::from_utf8(out).expect("only utf8 for common fields")
}
