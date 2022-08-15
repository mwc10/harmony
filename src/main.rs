fn print_usage() {
    println!("{} <INPUT DIRECTORY>", env!("CARGO_BIN_NAME"))
}

fn main() {
    let dir = std::env::args().nth(1);

    match dir.as_deref() {
        None => eprintln!("Missing file name"),
        Some("-h" | "--help" | "help") => print_usage(),
        Some(dir) => print_found_files(dir),
    }
}

fn print_found_files(dir: &str) {
    // for (p, info) in harmony::collect_harmony_datafiles(dir) {
    //     println!("{}\n{:#?}\n", p.display(), info);
    // }

    let files = harmony::iterate_harmony_datafiles(dir)
        .filter(|m| m.population.is_some())
        .collect::<Vec<_>>();
    harmony::combine_files(&files);
}
