//! Núcleo do arbor, sem dependência de interface: listagem, ordenação, busca fuzzy e operações de arquivo.
//! Tudo aqui é testável sem janela e é o que o `arbor-bench` mede.

pub mod archive;
pub mod entry;
pub mod fuzzy;
pub mod ops;
pub mod scan;

pub use entry::{compare, human_count, human_size, human_time, sort_entries, Entry, Ext, Sort, SortKey};
pub use scan::{scan, scan_all, Snapshot};
