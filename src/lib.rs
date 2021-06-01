//! The standard API includes: the [`malloc`], [`calloc`], [`realloc`], and
//! [`free`], which conform to to ISO/IEC 9899:1990 (“ISO C90”),
//! [`posix_memalign`] which conforms to conforms to POSIX.1-2016, and
//! [`aligned_alloc`].

use backtrace::Frame;
use std::hash::{Hash, Hasher};
use std::sync::RwLock;

mod shim;

pub const MAX_DEPTH: usize = 32;

lazy_static::lazy_static! {
    pub(crate) static ref HEAP_PROFILER: RwLock<Profiler> = RwLock::new(Profiler::new());
}

struct Profiler {
    collector: pprof::Collector<StaticBacktrace>,
}

impl Profiler {
    fn new() -> Self {
        Self {
            collector: pprof::Collector::new().unwrap(),
        }
    }
}

struct StaticBacktrace {
    frames: [Frame; MAX_DEPTH],
    size: usize,
}

impl StaticBacktrace {
    unsafe fn new() -> Self {
        Self {
            frames: std::mem::MaybeUninit::uninit().assume_init(),
            size: 0,
        }
    }

    unsafe fn push(&mut self, frame: &Frame) -> bool {
        self.frames[self.size] = frame.clone();
        self.size += 1;
        self.size < MAX_DEPTH
    }

    fn iter<'a>(&'a self) -> StaticBacktraceIterator<'a> {
        StaticBacktraceIterator(self, 0)
    }
}

impl Hash for StaticBacktrace {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.iter()
            .for_each(|frame| frame.symbol_address().hash(state));
    }
}

impl PartialEq for StaticBacktrace {
    fn eq(&self, other: &Self) -> bool {
        Iterator::zip(self.iter(), other.iter())
            .map(|(s1, s2)| s1.symbol_address() == s2.symbol_address())
            .all(|equal| equal)
    }
}

impl Eq for StaticBacktrace {}

struct StaticBacktraceIterator<'a>(&'a StaticBacktrace, usize);

impl<'a> Iterator for StaticBacktraceIterator<'a> {
    type Item = &'a Frame;

    fn next(&mut self) -> Option<Self::Item> {
        if self.1 < self.0.size {
            let res = Some(&self.0.frames[self.1]);
            self.1 += 1;
            res
        } else {
            None
        }
    }
}

impl From<StaticBacktrace> for pprof::Frames {
    fn from(bt: StaticBacktrace) -> Self {
        let frames = bt
            .iter()
            .map(|frame| {
                let mut symbols: Vec<pprof::Symbol> = Vec::new();
                backtrace::resolve_frame(frame, |symbol| {
                    if let Some(name) = symbol.name() {
                        let name = format!("{:#}", name);
                        symbols.push(pprof::Symbol {
                            name: Some(name.as_bytes().to_vec()),
                            addr: None,
                            lineno: None,
                            filename: None,
                        })
                    }
                });
                symbols
            })
            .collect();
        Self {
            frames,
            thread_name: "".to_string(),
            thread_id: 0,
        }
    }
}

pub(crate) unsafe fn track_allocated(size: usize) {
    println!("allocated {}", size);

    let mut bt = StaticBacktrace::new();
    backtrace::trace(|frame| bt.push(frame));

    /*
    for frame in bt.iter() {
        backtrace::resolve_frame(frame, |symbol| {
            if let Some(name) = symbol.name() {
                println!("{:#}", name);
            }
        });
    }
     */

    let mut profiler = HEAP_PROFILER.write();
    let profiler = profiler.as_mut().unwrap();
    profiler.collector.add(bt, size as isize).unwrap();
}

pub fn demo() {
    let profiler = HEAP_PROFILER.read().unwrap();
    for entry in profiler.collector.try_iter().unwrap() {
        print!("<root>");
        for frame in entry.item.iter() {
            backtrace::resolve_frame(frame, |symbol| {
                if let Some(name) = symbol.name() {
                    print!(";{:#}", name);
                }
            });
        }
        println!(" {}", entry.count);
    }

    println!("demo");
}
