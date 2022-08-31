#![windows_subsystem = "windows"]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use druid::im::{HashSet, Vector};
use druid::widget::{
    Align, Button, Checkbox, Either, Flex, Label, LineBreaking, List, MainAxisAlignment, Scroll,
    SizedBox, Spinner,
};
use druid::{
    commands, lens, AppLauncher, Command, Data, FileDialogOptions, FileSpec, Lens, LensExt,
    LocalizedString, Target, TextAlignment, UnitPoint, Widget, WidgetExt, WindowDesc,
};
use harmony::HarmonyMetadata;

mod cmd;
mod delegate;

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
        .window_size((800.0, 650.0));
    let data = State::default();

    AppLauncher::with_window(main_window)
        .delegate(delegate::Delegate)
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

fn el_open_file_picker(title: &str) -> impl Widget<State> {
    let open_dialog_options = FileDialogOptions::new()
        .select_directories()
        .name_label("Directory with Exported Harmony Data")
        .title("Select a directory to combine exported Harmony data")
        .button_text("Select");

    Button::new(title).on_click(move |ctx, _, _| {
        ctx.submit_command(commands::SHOW_OPEN_PANEL.with(open_dialog_options.clone()))
    })
}

fn ui_picker() -> impl Widget<State> {
    let instructions = Label::new("Select a directory containing exported Harmony data:");
    let open_btn = el_open_file_picker("Select Data");

    let mut col = Flex::column();
    col.add_child(instructions);
    col.add_default_spacer();
    col.add_child(open_btn);

    Align::centered(col)
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
    let toggle_on = Button::new("Select All Files")
        .on_click(|_, s: &mut State, _| s.found_files.iter_mut().for_each(|f| f.include = true));
    let toggle_off = Button::new("Deselect All Files")
        .on_click(|_, s: &mut State, _| s.found_files.iter_mut().for_each(|f| f.include = false));
    let all_toggles = Flex::row()
        .with_child(toggle_on)
        .with_default_spacer()
        .with_child(toggle_off);

    // todo: constant for naming None population
    let pop_toggle = List::new(|| {
        Button::dynamic(|s: &Arc<str>, _| s.to_string()).on_click(|ctx, s, _| {
            let cmd = Command::new(crate::cmd::FILTER_POP, s.clone(), Target::Auto);
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

    let files = Scroll::new(List::new(el_plate_selector).with_spacing(4.0))
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
    .on_click(|ctx, _, _| ctx.submit_command(crate::cmd::START_COMBINE))
    .disabled_if(|s: &State, _| s.output.is_none());

    let try_again = el_open_file_picker("Pick Different Directory to Search");
    let table_title = Label::new("Choose Files to Combine").with_text_size(22.0);

    Flex::column()
        .with_default_spacer()
        .with_child(info)
        .with_default_spacer()
        .with_child(try_again)
        .with_spacer(18.0)
        .with_child(table_title)
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

fn el_plate_selector() -> impl Widget<(usize, FileInfo)> {
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
        let notice = Label::dynamic(|s: &State, _| {
            if let Some(outpath) = s.output.as_deref() {
                format!("Finished\nSaved to:\n{}", outpath.display())
            } else {
                format!("Finished")
            }
        })
        .with_text_alignment(TextAlignment::Center)
        .with_line_break_mode(LineBreaking::WordWrap);
        let back = Button::new("Go Back to Combine Other Files")
            .on_click(|_, s: &mut State, _| s.combining = Combining::Not);
        let restart = Button::new("Choose Another Starting Directory")
            .on_click(|_, s: &mut State, _| *s = State::default());

        Flex::column()
            .with_child(notice)
            .with_default_spacer()
            .with_child(back)
            .with_default_spacer()
            .with_child(restart)
            .center()
    };

    Either::new(|s, _| s.combining == Combining::Running, running, finished)
}

const fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
