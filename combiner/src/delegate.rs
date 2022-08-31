use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::Arc;

use druid::{commands, AppDelegate, Command, DelegateCtx, Env, Handled, Target};
use harmony::HarmonyMetadata;

use crate::cmd::*;
use crate::Combining;
use crate::FileInfo;
use crate::State;

pub(crate) struct Delegate;

impl AppDelegate<State> for Delegate {
    fn command(
        &mut self,
        ctx: &mut DelegateCtx,
        _target: Target,
        cmd: &Command,
        data: &mut State,
        _env: &Env,
    ) -> Handled {
        if let Some(file_info) = cmd.get(commands::OPEN_FILE) {
            if file_info.path.is_dir() {
                let sink = ctx.get_external_handle();
                let dir = file_info.path.clone();
                // restart the whole chain by discarding any prior data
                // todo: druid_enum ...
                *data = State::default();
                data.input_dir = Some(dir.clone());
                std::thread::spawn(move || find_harmony_files(dir, sink));
            }
            Handled::Yes
        } else if let Some(info) = cmd.get(FOUND_FILE).cloned() {
            // not perfect for utf8, but it hopefully it's always be longer than
            // the number of graphemes
            let n = info.plate_name.len();
            data.longest_pname = data.longest_pname.max(n);
            data.found_pops.insert(
                info.population
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| Arc::from("Well Data")),
            );
            data.found_files.push_back(info);

            Handled::Yes
        } else if let Some(files) = cmd.get(FINISHED_SEARCHING).cloned() {
            data.files = Some(files);
            Handled::Yes
        } else if let Some(pop) = cmd.get(FILTER_POP) {
            data.found_files.iter_mut().for_each(|f| {
                f.include = if pop.as_ref() == "Well Data" {
                    f.population.is_none()
                } else {
                    f.population.as_ref().map_or(false, |p| p == pop)
                }
            });
            Handled::Yes
        } else if let Some(f) = cmd.get(commands::SAVE_FILE_AS) {
            data.output = Some(f.path.clone());
            Handled::Yes
        } else if cmd.is(START_COMBINE) {
            let files = data.files.as_ref().map(|fs| {
                fs.iter()
                    .zip(data.found_files.iter().map(|f| f.include))
                    .filter(|(_, inc)| *inc)
                    .map(|(md, _)| md.clone())
                    .collect::<Vec<_>>()
            });
            match (files, data.output.as_ref()) {
                (Some(fs), Some(out)) => {
                    data.combining = Combining::Running;
                    let sink = ctx.get_external_handle();
                    let out = out.clone();
                    std::thread::spawn(move || combine_harmony_files(out, fs, sink));
                }
                _ => (),
            }
            Handled::Yes
        } else if cmd.is(FINISH_COMBINE) {
            data.combining = Combining::Completed;
            Handled::Yes
        } else {
            Handled::No
        }
    }
}

fn find_harmony_files(dir: PathBuf, sink: druid::ExtEventSink) {
    // probably better to switch to an im::Vector to provide realtime updates
    let mut out = Vec::new();
    for md in harmony::iterate_harmony_datafiles(dir) {
        let info = FileInfo {
            plate_name: md.plate_name.clone(),
            measurement: md.measurement,
            evaluation: md.evaluation,
            population: md.population.clone(),
            include: true,
        };

        sink.submit_command(FOUND_FILE, info, Target::Auto)
            .expect("please don't panic I don't know what I'm doing");
        out.push(md);
    }

    sink.submit_command(FINISHED_SEARCHING, Arc::from(out), Target::Auto)
        .expect("please work");
}

// todo: create error
fn combine_harmony_files(out: PathBuf, files: Vec<HarmonyMetadata>, sink: druid::ExtEventSink) {
    let wtr = File::create(out)
        .map(BufWriter::new)
        .expect("Create file for output");
    harmony::combine_files(wtr, &files).expect("Combine data into output");

    sink.submit_command(FINISH_COMBINE, (), Target::Auto)
        .expect("send finish combine cmd");
}
