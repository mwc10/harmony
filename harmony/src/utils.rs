use std::{
    collections::HashSet,
    fs::File,
    io::{self, BufRead, BufReader, Lines},
    path::Path,
    sync::Arc,
};

pub(crate) fn read_lines<P: AsRef<Path>>(p: P) -> io::Result<Lines<BufReader<File>>> {
    File::open(p).map(|f| BufReader::new(f).lines())
}

pub(crate) struct StrIntern(HashSet<Arc<str>>);

impl StrIntern {
    pub(crate) fn new() -> Self {
        Self(HashSet::with_capacity(0x100))
    }
    pub(crate) fn get(&mut self, s: &str) -> Arc<str> {
        if let Some(interned) = self.0.get(s) {
            Arc::clone(interned)
        } else {
            let interned = Arc::from(s);
            self.0.insert(Arc::clone(&interned));
            interned
        }
    }
}
