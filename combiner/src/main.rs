#![windows_subsystem = "windows"]

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use druid::im::{HashSet, Vector};
use druid::widget::{
    Align, Button, Checkbox, Either, Flex, Label, LineBreaking, List, MainAxisAlignment, Scroll,
    SizedBox, Spinner,
};
use druid::{
    commands, lens, AppDelegate, AppLauncher, Command, Data, DelegateCtx, Env, FileDialogOptions,
    FileSpec, Handled, Lens, LensExt, LocalizedString, Selector, Target, UnitPoint, Widget,
    WidgetExt, WindowDesc,
};
use harmony::HarmonyMetadata;

struct Delegate;

#[derive(Clone, Data, Lens, Default)]
struct State {
    #[data(same_fn = "PartialEq::eq")]
    input_dir: Option<PathBuf>,
    found_files: Vector<FileInfo>,
    found_pops: HashSet<Arc<str>>,
    longest_pname: usize,
    files: Option<Arc<[HarmonyMetadata]>>,
    #[data(same_fn = "PartialEq::eq")]
    output: Option<PathBuf>,
    combining: Combining,
}

impl State {
    fn count_included_files(&self) -> usize {
        self.found_files.iter().filter(|f| f.include).count()
    }
}

#[derive(Clone, Data, Lens)]
struct FileInfo {
    plate_name: String,
    measurement: u32,
    evaluation: u32,
    population: Option<Arc<str>>,
    include: bool,
}

#[derive(Clone, Data, PartialEq, Eq, Copy)]
enum Combining {
    Not,
    Running,
    Completed,
}

impl Default for Combining {
    fn default() -> Self {
        Self::Not
    }
}

fn main() {
    let main_window = WindowDesc::new(ui_builder())
        .title(
            LocalizedString::new("harmony-data-combiner").with_placeholder("Harmony Data Combiner"),
        )
        .window_size((800.0, 600.0));
    let data = State::default();

    AppLauncher::with_window(main_window)
        .delegate(Delegate)
        .log_to_console()
        .launch(data)
        .expect("launch failed");
}

fn ui_builder() -> impl Widget<State> {
    Either::new(
        |data, _env| data.input_dir.is_none(),
        ui_picker(),
        Either::new(
            |state, _env| state.files.is_none(),
            ui_finding_files(),
            Either::new(
                |state, _env| state.combining == Combining::Not,
                ui_select_files(),
                ui_running_combiner(),
            ),
        ),
    )
}

fn ui_picker() -> impl Widget<State> {
    let open_dialog_options = FileDialogOptions::new()
        .select_directories()
        .name_label("Directory with Exported Harmony Data")
        .title("Select a directory to combine exported Harmony data")
        .button_text("Select");

    let open = Button::new("Select Data").on_click(move |ctx, _, _| {
        ctx.submit_command(commands::SHOW_OPEN_PANEL.with(open_dialog_options.clone()))
    });

    let instructions = Label::new("Select a directory containing exported Harmony data:");

    let mut col = Flex::column();
    col.add_child(instructions);
    col.add_default_spacer();
    col.add_child(open);

    Align::centered(col)
}

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
                data.input_dir = Some(dir.clone());
                // start command here?
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

fn ui_finding_files() -> impl Widget<State> {
    let count = Label::dynamic(|state: &State, _| {
        format!(
            "found {} file{}",
            state.found_files.len(),
            plural(state.found_files.len())
        )
    });
    let spinner = SizedBox::new(Spinner::new()).width(100.0).height(100.0);

    let col = Flex::column()
        .with_default_spacer()
        .with_child(Label::new("Searching for files..."))
        .with_default_spacer()
        .with_child(spinner)
        .with_default_spacer()
        .with_child(count);

    Align::vertical(UnitPoint::CENTER, col)
}

fn ui_select_files() -> impl Widget<State> {
    let info = Label::dynamic(|state: &State, _| {
        let n = state.found_files.len();
        if let Some(path) = state.input_dir.as_deref().map(Path::display) {
            format!("Found {} file{} in:\n{}", n, plural(n), path)
        } else {
            format!("Found {} file{}", n, plural(n))
        }
    })
    .with_line_break_mode(LineBreaking::WordWrap);

    // all / none toggles
    let toggle_on = Button::new("Select All")
        .on_click(|_, s: &mut State, _| s.found_files.iter_mut().for_each(|f| f.include = true));
    let toggle_off = Button::new("Deselect All")
        .on_click(|_, s: &mut State, _| s.found_files.iter_mut().for_each(|f| f.include = false));
    let all_toggles = Flex::row()
        .with_child(toggle_on)
        .with_default_spacer()
        .with_child(toggle_off);

    // todo: constant for naming None population
    let pop_toggle = List::new(|| {
        Button::dynamic(|s: &Arc<str>, _| s.to_string()).on_click(|ctx, s, _| {
            let cmd = Command::new(FILTER_POP, s.clone(), Target::Auto);
            ctx.submit_command(cmd)
        })
    })
    .with_spacing(2.0)
    .horizontal()
    .lens(State::found_pops.map(
        |hs| {
            let mut v = hs.iter().cloned().collect::<Vector<_>>();
            v.sort();
            v
        },
        |vs, hs| *vs = hs.into(),
    ));

    let files = Scroll::new(List::new(plate_selector).with_spacing(4.0))
        .lens((State::longest_pname, State::found_files));

    let tsv = FileSpec::new("TSV File", &["tsv", "txt"]);
    let default_name = "combined-data.tsv";
    let save_options = FileDialogOptions::new()
        .allowed_types(vec![tsv])
        .default_type(tsv)
        .default_name(default_name)
        .name_label("Output Filename")
        .title("Select file for output")
        .button_text("Select");
    let save = Button::new("Select File for Output").on_click(move |ctx, _, _| {
        ctx.submit_command(commands::SHOW_SAVE_PANEL.with(save_options.clone()))
    });

    let display_out = Label::dynamic(|s: &State, _| {
        if let Some(p) = s.output.as_deref() {
            format!("{}", p.display())
        } else {
            format!("Output not selected")
        }
    })
    .with_line_break_mode(LineBreaking::WordWrap);

    let run = Button::dynamic(|s: &State, _| {
        let n = s.count_included_files();
        format!("Combine {} File{}", n, plural(n))
    })
    .on_click(|ctx, _, _| ctx.submit_command(START_COMBINE))
    .disabled_if(|s: &State, _| s.output.is_none());

    Flex::column()
        .with_default_spacer()
        .with_child(info)
        .with_default_spacer()
        .with_child(all_toggles)
        .with_spacer(0.5)
        .with_child(pop_toggle)
        .with_default_spacer()
        .with_flex_child(files, 1.0)
        .with_spacer(25.0)
        .with_child(save)
        .with_default_spacer()
        .with_child(display_out)
        .with_default_spacer()
        .with_child(run)
        .with_default_spacer()
        .main_axis_alignment(MainAxisAlignment::SpaceEvenly)
}

const FILTER_POP: Selector<Arc<str>> = Selector::new("app.files.filter-population");
const FOUND_FILE: Selector<FileInfo> = Selector::new("app.harmony.found-file");
const FINISHED_SEARCHING: Selector<Arc<[HarmonyMetadata]>> =
    Selector::new("app.harmony.search-done");
const START_COMBINE: Selector<()> = Selector::new("app.harmony.combine-start");
const FINISH_COMBINE: Selector<()> = Selector::new("app.harmony.combine-finish");

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

// fn combine_harmony_files(md: Arc<[HarmonyMetadata], sink: druid::ExtEventSink)

fn plate_selector() -> impl Widget<(usize, FileInfo)> {
    let include = Checkbox::new("").lens(lens!((usize, FileInfo), 1).then(FileInfo::include));
    let label = Label::dynamic(move |(w, i): &(usize, FileInfo), _| {
        format!(
            "{:<width$}\t|   M{}   |   E{}   |   {}",
            i.plate_name,
            i.measurement,
            i.evaluation,
            i.population.as_deref().unwrap_or("Well Data"),
            width = w
        )
    })
    .on_click(|_ctx, state, _env| state.1.include = !state.1.include);

    Flex::row()
        .with_child(include)
        .with_default_spacer()
        .with_child(label)
}

fn ui_running_combiner() -> impl Widget<State> {
    let running = {
        let spinner = SizedBox::new(Spinner::new()).width(100.0).height(100.0);
        let label = Label::dynamic(|s: &State, _| {
            let n = s.found_files.iter().filter(|f| f.include).count();
            format!("Combining {} file{}", n, plural(n))
        });
        Flex::column()
            .with_child(spinner)
            .with_default_spacer()
            .with_child(label)
            .center()
    };
    let finished = {
        let notice = Label::new("Combining Finished! Start Again? Set Timer to go back?");
        let back =
            Button::new("Go Back").on_click(|_, s: &mut State, _| s.combining = Combining::Not);

        Flex::column()
            .with_child(notice)
            .with_default_spacer()
            .with_child(back)
            .center()
    };

    Either::new(|s, _| s.combining == Combining::Running, running, finished)
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

const fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
