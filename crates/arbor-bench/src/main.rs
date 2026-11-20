//! Benchmarks reproduzíveis do arbor. Sem janela: mede o que decide se a interface trava ou não.
//!
//! ```text
//! arbor-bench gen    <pasta> [n]         cria n arquivos (padrão 100 mil) com nomes realistas
//! arbor-bench suite  <pasta> [--check]   mede primeiro quadro, listagem, ordenação e filtro; --check falha se passar das metas
//! arbor-bench list   <pasta>             lista + metadados + ordem natural (para comparar com ls, eza, find no hyperfine)
//! arbor-bench churn  <pasta> [segundos]  pasta que muda sem parar: leituras e CPU gastos pela política antiga (250 ms) e pela atual
//! arbor-bench filter <consulta>          filtro fuzzy nos nomes lidos da entrada padrão (para comparar com fzf --filter)
//! ```

use arbor_core::{fuzzy, scan, scan_all, sort_entries, Entry, Sort, SortKey};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// mimalloc: a listagem cria centenas de milhares de alocações pequenas em várias threads; com o
/// alocador do sistema elas disputam o mesmo lock e a pasta de 100 mil itens leva 4× mais.
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

const DEFAULT_N: usize = 100_000;
const RUNS: usize = 9;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).map(String::as_str);
    match arg(0) {
        Some("gen") => {
            let dir = PathBuf::from(arg(1).unwrap_or("bench-100k"));
            let n = arg(2).and_then(|s| s.parse().ok()).unwrap_or(DEFAULT_N);
            generate(&dir, n);
            ExitCode::SUCCESS
        }
        Some("list") => {
            let snap = scan_all(Path::new(arg(1).unwrap_or(".")), Sort::default());
            if let Some(e) = snap.error {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{}", snap.entries.len());
            ExitCode::SUCCESS
        }
        Some("filter") => {
            let names: Vec<String> = std::io::stdin().lock().lines().map_while(Result::ok).collect();
            let hits = fuzzy::rank_slice(arg(1).unwrap_or(""), &names, |s| s.as_str());
            let mut out = std::io::BufWriter::new(std::io::stdout().lock());
            for i in hits {
                let _ = writeln!(out, "{}", names[i]);
            }
            ExitCode::SUCCESS
        }
        Some("churn") => {
            let dir = PathBuf::from(arg(1).unwrap_or("bench-100k"));
            let secs = arg(2).and_then(|s| s.parse().ok()).unwrap_or(6);
            churn(&dir, secs)
        }
        Some("suite") => {
            let dir = PathBuf::from(arg(1).unwrap_or("bench-100k"));
            if !dir.is_dir() {
                generate(&dir, DEFAULT_N);
            }
            let check = args.iter().any(|a| a == "--check");
            suite(&dir, check)
        }
        _ => {
            eprintln!("uso: arbor-bench gen <pasta> [n] | suite <pasta> [--check] | list <pasta> | filter <consulta>");
            ExitCode::from(2)
        }
    }
}

/// Nomes no estilo de uma pasta `target/` de Rust: prefixo de crate, hash e extensão, sem ordem.
fn generate(dir: &Path, n: usize) {
    const PRE: [&str; 20] = [
        "iced", "wgpu", "tokio", "vislo", "image", "nucleo", "notify", "serde", "winit", "cosmic", "rayon", "smallvec", "naga", "ttf",
        "glyphon", "bytemuck", "png", "zune", "half", "lyon",
    ];
    const EXT: [&str; 5] = ["rlib", "rmeta", "d", "o", "json"];
    std::fs::create_dir_all(dir).expect("criar pasta do benchmark");
    let existing = std::fs::read_dir(dir).map(|d| d.count()).unwrap_or(0);
    if existing >= n {
        eprintln!("{} já tem {existing} itens", dir.display());
        return;
    }
    let mut h: u32 = 2_166_136_261;
    let start = Instant::now();
    for k in 0..n {
        h ^= h >> 15;
        h = h.wrapping_mul(2_246_822_507);
        h ^= h >> 13;
        h = h.wrapping_mul(3_266_489_909);
        h ^= h >> 16;
        let name = format!("{}-{:08x}-{k}.{}", PRE[k % 20], h, EXT[(k >> 3) % 5]);
        std::fs::File::create(dir.join(name)).expect("criar arquivo");
    }
    for d in 0..64 {
        let _ = std::fs::create_dir(dir.join(format!("dir-{d}")));
    }
    eprintln!("{n} arquivos criados em {} ({:.1} s)", dir.display(), start.elapsed().as_secs_f64());
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn time<T>(runs: usize, mut f: impl FnMut() -> T) -> Duration {
    let _ = f(); // aquece o cache de diretório do sistema, como numa segunda abertura
    median(
        (0..runs)
            .map(|_| {
                let t = Instant::now();
                std::hint::black_box(f());
                t.elapsed()
            })
            .collect(),
    )
}

/// O jeito comum: `read_dir` + `metadata` numa thread e `sort_by` com comparação natural a cada passo.
/// É o que o protótipo fazia e o que a maioria dos explorers faz.
fn naive(dir: &Path) -> Vec<(String, u64, bool)> {
    let mut v: Vec<(String, u64, bool)> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((e.file_name().to_string_lossy().into_owned(), m.len(), m.is_dir()))
        })
        .collect();
    v.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| natord::compare_ignore_case(&a.0, &b.0)));
    v
}

struct Row {
    name: &'static str,
    value: Duration,
    target: Option<Duration>,
    note: String,
}

fn suite(dir: &Path, check: bool) -> ExitCode {
    let slack: f64 = std::env::var("ARBOR_BENCH_SLACK").ok().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let ms = |m: u64| Duration::from_millis(m);

    let first = time(RUNS, || {
        let mut got = Duration::ZERO;
        let start = Instant::now();
        scan(dir, Sort::default(), |_| {
            got = start.elapsed();
            false
        });
        got
    });
    let mut items = 0;
    let full = time(RUNS, || {
        let s = scan_all(dir, Sort::default());
        items = s.entries.len();
        s
    });
    let baseline = time(RUNS.min(5), || naive(dir));

    let snap = scan_all(dir, Sort::default());
    let entries: Vec<Entry> = snap.entries.as_ref().clone();
    let resort = time(RUNS, || {
        let mut v = entries.clone();
        let t = Instant::now();
        sort_entries(&mut v, Sort { key: SortKey::Size, desc: true });
        t.elapsed()
    });
    let resort = {
        // descontar o clone: mede só a ordenação
        let mut v: Vec<Duration> = (0..RUNS)
            .map(|_| {
                let mut c = entries.clone();
                let t = Instant::now();
                sort_entries(&mut c, Sort { key: SortKey::Size, desc: true });
                t.elapsed()
            })
            .collect();
        v.push(resort);
        median(v)
    };

    let queries = ["v", "vi", "vis", "visl", "vislo", "rs", "tok3", "png-1"];
    let per_key: Vec<Duration> = queries.iter().map(|q| time(RUNS, || fuzzy::rank_slice(q, &entries, |e| e.name.as_str()))).collect();
    let worst_key = *per_key.iter().max().unwrap();
    let typical_key = median(per_key);

    let bytes: usize =
        entries.iter().map(|e| std::mem::size_of::<Entry>() + e.name.capacity() + e.path.as_os_str().len() + e.key.len()).sum();

    let rows = [
        Row { name: "Primeiro quadro (primeiros 256 itens, ordenados)", value: first, target: Some(ms(50)), note: String::new() },
        Row {
            name: "Pasta completa: listar, ler metadados e ordenar",
            value: full,
            target: Some(ms(400)),
            note: format!("{:.1}× mais rápido que o jeito comum ({})", baseline.as_secs_f64() / full.as_secs_f64(), fmt(baseline)),
        },
        Row { name: "Reordenar por tamanho", value: resort, target: Some(ms(50)), note: String::new() },
        Row {
            name: "Filtro fuzzy por tecla (mediana)",
            value: typical_key,
            target: Some(ms(8)),
            note: format!("pior consulta {}", fmt(worst_key)),
        },
    ];

    println!(
        "### arbor-bench · {} itens · {} · {} núcleos\n",
        items,
        std::env::consts::OS,
        std::thread::available_parallelism().map_or(1, |n| n.get())
    );
    println!("| Medida | arbor | Meta | |\n|---|---:|---:|---|");
    let mut failed = false;
    for r in &rows {
        let target = r.target.map(|t| t.mul_f64(slack));
        let ok = target.is_none_or(|t| r.value <= t);
        failed |= !ok;
        let mark = if ok { "✅" } else { "❌" };
        let note = if r.note.is_empty() { String::new() } else { format!(" {}", r.note) };
        println!(
            "| {} | **{}** | {} | {mark}{note} |",
            r.name,
            fmt(r.value),
            r.target.map(|t| format!("< {}", fmt(t))).unwrap_or_default()
        );
    }
    println!("| Memória da lista ({} itens) | **{:.1} MB** | < 40 MB (app inteiro) | |", items, bytes as f64 / 1_048_576.0);
    println!();
    if check && failed {
        eprintln!("uma ou mais metas de desempenho falharam (folga ARBOR_BENCH_SLACK={slack})");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn fmt(d: Duration) -> String {
    let ms = d.as_secs_f64() * 1000.0;
    if ms < 10.0 {
        format!("{ms:.2} ms")
    } else {
        format!("{ms:.0} ms")
    }
}

/// CPU (usuário + sistema) do processo, em segundos; só no Linux, onde o benchmark de CI roda.
fn cpu_seconds() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let f: Vec<&str> = stat.rsplit_once(')').map_or("", |(_, r)| r).split_whitespace().collect();
    let ticks = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    (ticks(11) + ticks(12)) / 100.0
}

/// Alguém escreve na pasta o tempo todo (cópia grande, build, download); a interface relê a pasta a cada
/// aviso do disco. Compara reler 250 ms depois do aviso (antes) com esperar também 4× o custo da leitura (agora).
fn churn(dir: &Path, secs: u64) -> ExitCode {
    if !dir.is_dir() {
        generate(dir, DEFAULT_N);
    }
    let mut rows = Vec::new();
    for (name, throttled) in [("250 ms fixos (antes)", false), ("espera 4× a leitura (agora)", true)] {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let writer = {
            let (stop, dir) = (stop.clone(), dir.to_path_buf());
            std::thread::spawn(move || {
                let mut i = 0u64;
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = std::fs::write(dir.join(format!("churn-{i}.tmp")), b"x");
                    i += 1;
                    std::thread::sleep(Duration::from_millis(2));
                }
                for j in 0..i {
                    let _ = std::fs::remove_file(dir.join(format!("churn-{j}.tmp")));
                }
            })
        };
        let (cpu0, start) = (cpu_seconds(), Instant::now());
        let (mut scans, mut took_total, mut settled) = (0u32, Duration::ZERO, Instant::now());
        let mut gap = Duration::ZERO;
        while start.elapsed() < Duration::from_secs(secs) {
            std::thread::sleep(Duration::from_millis(250)); // o debounce do watcher
            if throttled {
                std::thread::sleep(gap.saturating_sub(settled.elapsed()));
            }
            let t = Instant::now();
            let snap = scan_all(dir, Sort::default());
            gap = arbor_core::scan::refresh_gap(snap.elapsed);
            took_total += t.elapsed();
            settled = Instant::now();
            scans += 1;
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = writer.join();
        let wall = start.elapsed().as_secs_f64();
        rows.push(format!(
            "| {name} | {scans} | {:.1} s | {:.0} % de 1 núcleo |",
            took_total.as_secs_f64(),
            (cpu_seconds() - cpu0) / wall * 100.0
        ));
    }
    println!(
        "### churn · {} · {secs} s de escrita contínua\n\n| política | leituras | tempo lendo | CPU |\n|---|---:|---:|---:|",
        dir.display()
    );
    rows.iter().for_each(|r| println!("{r}"));
    ExitCode::SUCCESS
}
