use std::sync::Arc;

use druid::Selector;
use harmony::HarmonyMetadata;

use crate::FileInfo;

pub(crate) const FILTER_POP: Selector<Arc<str>> = Selector::new("app.files.filter-population");
pub(crate) const FOUND_FILE: Selector<FileInfo> = Selector::new("app.harmony.found-file");
pub(crate) const FINISHED_SEARCHING: Selector<Arc<[HarmonyMetadata]>> =
    Selector::new("app.harmony.search-done");
pub(crate) const START_COMBINE: Selector<()> = Selector::new("app.harmony.combine-start");
pub(crate) const FINISH_COMBINE: Selector<()> = Selector::new("app.harmony.combine-finish");
