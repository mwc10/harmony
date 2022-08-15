use std::{
    collections::HashSet,
    fs::File,
    io::{self, BufRead, BufReader, Lines},
    path::Path,
    rc::Rc,
};

pub(crate) fn read_lines<P: AsRef<Path>>(p: P) -> io::Result<Lines<BufReader<File>>> {
    File::open(p).map(|f| BufReader::new(f).lines())
}

pub(crate) struct StrIntern(HashSet<Rc<str>>);

impl StrIntern {
    pub(crate) fn new() -> Self {
        Self(HashSet::with_capacity(0x100))
    }
    pub(crate) fn get(&mut self, s: &str) -> Rc<str> {
        if let Some(interned) = self.0.get(s) {
            Rc::clone(interned)
        } else {
            let interned = Rc::from(s);
            self.0.insert(Rc::clone(&interned));
            interned
        }
    }
}
