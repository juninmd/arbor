//! Estado git da pasta aberta: branch, worktree, à frente/atrás, alterações e linhas do diff, e a marca
//! de cada item da pasta. Usa o `git` instalado (sem shell, sem travar o índice); sem git, nada aparece.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Mudança de um item, da menos para a mais importante: a pasta mostra a pior entre as de dentro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Change {
    Untracked,
    Modified,
    Added,
    Renamed,
    Deleted,
    Conflict,
}

impl Change {
    pub fn letter(self) -> &'static str {
        match self {
            Change::Untracked => "U",
            Change::Modified => "M",
            Change::Added => "A",
            Change::Renamed => "R",
            Change::Deleted => "D",
            Change::Conflict => "!",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Change::Untracked => "Novo, fora do git",
            Change::Modified => "Modificado",
            Change::Added => "Adicionado",
            Change::Renamed => "Renomeado",
            Change::Deleted => "Excluído",
            Change::Conflict => "Em conflito",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GitInfo {
    /// Nome da branch, ou "HEAD @abc1234" quando desanexado.
    pub branch: String,
    /// Nome do worktree vinculado (a pasta dele), quando não é o clone principal.
    pub worktree: Option<String>,
    pub upstream: bool,
    pub ahead: u32,
    pub behind: u32,
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
    pub conflicts: usize,
    pub added_lines: u64,
    pub removed_lines: u64,
    /// Endereço web do remoto (origin, ou o primeiro): só http(s), sem usuário nem senha.
    pub remote: Option<String>,
    /// Itens diretos da pasta aberta com mudança (pasta: a pior mudança dentro dela).
    pub children: HashMap<PathBuf, Change>,
}

impl GitInfo {
    pub fn changed(&self) -> usize {
        self.staged + self.unstaged + self.untracked + self.conflicts
    }
}

/// Só chama o git se alguma pasta acima tem `.git`: abrir pastas comuns não pode criar processos.
pub fn find_root(dir: &Path) -> Option<&Path> {
    dir.ancestors().find(|d| crate::props::is_repo(d))
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["--no-optional-locks", "-c", "core.quotepath=off"])
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn read(dir: &Path) -> Option<GitInfo> {
    find_root(dir)?;
    let rev = git(dir, &["rev-parse", "--path-format=absolute", "--show-toplevel", "--git-dir", "--git-common-dir"])?;
    let mut lines = rev.lines();
    let (top, git_dir, common) = (PathBuf::from(lines.next()?), lines.next()?.to_string(), lines.next()?.to_string());
    let status = git(&top, &["status", "--porcelain=v2", "--branch", "--untracked-files=normal"])?;
    let numstat = git(&top, &["diff", "--numstat", "HEAD"]).unwrap_or_default();
    // o git devolve caminhos reais; a pasta aberta pode ter chegado por um link (/tmp no macOS)
    let real = if cfg!(unix) { std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf()) } else { dir.to_path_buf() };
    let mut info = parse_status(&status, &top, &real);
    if real != dir {
        info.children = info.children.into_iter().filter_map(|(k, v)| Some((dir.join(k.strip_prefix(&real).ok()?), v))).collect();
    }
    (info.added_lines, info.removed_lines) = parse_numstat(&numstat);
    info.remote = git(&top, &["config", "--get-regexp", r"^remote\..*\.url$"]).and_then(|t| pick_remote(&t));
    if git_dir != common {
        info.worktree = top.file_name().map(|n| n.to_string_lossy().into_owned());
    }
    Some(info)
}

/// Escolhe o remoto `origin` (ou o primeiro com endereço web) na saída de `git config --get-regexp`.
fn pick_remote(config: &str) -> Option<String> {
    let urls: Vec<(&str, &str)> = config
        .lines()
        .filter_map(|l| l.split_once(' '))
        .filter_map(|(k, v)| Some((k.strip_prefix("remote.")?.strip_suffix(".url")?, v)))
        .collect();
    let origin = urls.iter().filter(|(n, _)| *n == "origin");
    origin.chain(urls.iter()).find_map(|(_, u)| web_url(u))
}

/// Endereço do remoto como página web: `git@host:o/r.git`, `ssh://`, `git://` e `https://` viram `https://host/o/r`.
/// Credenciais embutidas somem; caminhos locais e esquemas desconhecidos dão `None` (nunca abrimos `file:` ou `ext::`).
pub fn web_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let (scheme, host_path) = match raw.split_once("://") {
        Some((s @ ("http" | "https"), rest)) => (s, rest.to_string()),
        Some(("ssh" | "git", rest)) => ("https", strip_port(rest.to_string())),
        Some(_) => return None,
        None => {
            // scp: `[usuario@]host:caminho`; uma letra antes do ":" é unidade do Windows, "/" ou "." é caminho local
            let (host, path) = raw.split_once(':')?;
            if host.len() < 2 || host.starts_with(['/', '.', '\\', '-']) || host.contains('/') || path.starts_with(':') {
                return None;
            }
            ("https", format!("{host}/{}", path.trim_start_matches('/')))
        }
    };
    let host_path = host_path.rsplit_once('@').filter(|(u, _)| !u.contains('/')).map_or(host_path.as_str(), |(_, rest)| rest);
    let trimmed = host_path.trim_end_matches('/');
    let clean = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    let (host, path) = clean.split_once('/')?;
    (!host.is_empty() && !path.is_empty()).then(|| format!("{scheme}://{host}/{path}"))
}

/// `host:2222/caminho` → `host/caminho` (a porta do ssh não vale para a página web).
fn strip_port(rest: String) -> String {
    let at = rest.find('@').map_or(0, |i| i + 1);
    match (rest[at..].find(':'), rest[at..].find('/')) {
        (Some(c), Some(s)) if c < s => format!("{}{}", &rest[..at + c], &rest[at + s..]),
        _ => rest,
    }
}

/// Item direto de `dir` que contém `abs` (o próprio, ou a subpasta por onde passa).
fn child_of(dir: &Path, abs: &Path) -> Option<PathBuf> {
    let rest = abs.strip_prefix(dir).ok()?;
    rest.components().next().map(|c| dir.join(c))
}

fn parse_status(text: &str, top: &Path, dir: &Path) -> GitInfo {
    let mut info = GitInfo::default();
    let mut oid = String::new();
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("# branch.head ") {
            info.branch = h.to_string();
        } else if let Some(o) = line.strip_prefix("# branch.oid ") {
            oid = o.chars().take(7).collect();
        } else if line.starts_with("# branch.upstream ") {
            info.upstream = true;
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            let mut it = ab.split_whitespace();
            info.ahead = it.next().and_then(|a| a.trim_start_matches('+').parse().ok()).unwrap_or(0);
            info.behind = it.next().and_then(|b| b.trim_start_matches('-').parse().ok()).unwrap_or(0);
        } else {
            let Some((change, path)) = entry(line, &mut info) else { continue };
            if let Some(child) = child_of(dir, &top.join(path)) {
                let slot = info.children.entry(child).or_insert(change);
                *slot = (*slot).max(change);
            }
        }
    }
    if info.branch == "(detached)" {
        info.branch = format!("HEAD @{oid}");
    }
    info
}

/// Uma linha de arquivo do porcelain v2: conta nas somas e devolve a mudança e o caminho relativo.
fn entry<'a>(line: &'a str, info: &mut GitInfo) -> Option<(Change, &'a str)> {
    let kind = line.get(..2)?;
    match kind {
        "? " => {
            info.untracked += 1;
            Some((Change::Untracked, &line[2..]))
        }
        "u " => {
            info.conflicts += 1;
            Some((Change::Conflict, line.splitn(11, ' ').nth(10)?))
        }
        "1 " | "2 " => {
            let xy = line.get(2..4)?.as_bytes();
            let (x, y) = (xy[0], xy[1]);
            if x != b'.' {
                info.staged += 1;
            }
            if y != b'.' {
                info.unstaged += 1;
            }
            let path = if kind == "1 " { line.splitn(9, ' ').nth(8)? } else { line.splitn(10, ' ').nth(9)?.split('\t').next()? };
            let change = match (x, y) {
                (b'D', _) | (_, b'D') => Change::Deleted,
                (b'R' | b'C', _) => Change::Renamed,
                (b'A', _) => Change::Added,
                _ => Change::Modified,
            };
            Some((change, path))
        }
        _ => None,
    }
}

/// Soma de `git diff --numstat`: linhas adicionadas e removidas (binários vêm como "-" e não contam).
fn parse_numstat(text: &str) -> (u64, u64) {
    text.lines().fold((0, 0), |(a, r), line| {
        let mut it = line.split('\t');
        let add = it.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        let del = it.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        (a + add, r + del)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = "# branch.oid 1234567890abcdef\n\
# branch.head feat/visual\n\
# branch.upstream origin/feat/visual\n\
# branch.ab +2 -1\n\
1 .M N... 100644 100644 100644 aaa bbb src/app.rs\n\
1 M. N... 100644 100644 100644 aaa bbb src/view/props view.rs\n\
1 A. N... 000000 100644 100644 000 bbb README.md\n\
2 R. N... 100644 100644 100644 aaa bbb R100 docs/novo.md\tdocs/velho.md\n\
u UU N... 100644 100644 100644 100644 a b c Cargo.toml\n\
? notas.txt\n";

    #[test]
    fn status_counts_branch_and_marks_direct_children() {
        let top = Path::new("/r");
        let info = parse_status(STATUS, top, top);
        assert_eq!(info.branch, "feat/visual");
        assert!(info.upstream);
        assert_eq!((info.ahead, info.behind), (2, 1));
        assert_eq!((info.staged, info.unstaged, info.untracked, info.conflicts), (3, 1, 1, 1));
        // pasta com um modificado e um nome com espaço: a pasta herda a pior mudança de dentro
        assert_eq!(info.children[Path::new("/r/src")], Change::Modified);
        assert_eq!(info.children[Path::new("/r/docs")], Change::Renamed);
        assert_eq!(info.children[Path::new("/r/Cargo.toml")], Change::Conflict);
        assert_eq!(info.children[Path::new("/r/notas.txt")], Change::Untracked);
    }

    #[test]
    fn inside_a_subfolder_only_its_items_are_marked() {
        let info = parse_status(STATUS, Path::new("/r"), Path::new("/r/src"));
        assert_eq!(info.children.len(), 2, "README e docs ficam fora de /r/src: {:?}", info.children);
        assert_eq!(info.children[Path::new("/r/src/view")], Change::Modified);
        assert_eq!(info.changed(), 6, "a faixa ainda resume o repositório todo");
    }

    #[test]
    fn detached_head_shows_short_commit() {
        let info = parse_status("# branch.oid abcdef0123\n# branch.head (detached)\n", Path::new("/r"), Path::new("/r"));
        assert_eq!(info.branch, "HEAD @abcdef0");
        assert!(!info.upstream);
    }

    #[test]
    fn remote_becomes_a_web_link() {
        let ok = |a: &str, b: &str| assert_eq!(web_url(a).as_deref(), Some(b), "{a}");
        ok("git@github.com:juninmd/arbor.git", "https://github.com/juninmd/arbor");
        ok("https://github.com/juninmd/arbor.git", "https://github.com/juninmd/arbor");
        ok("https://user:tok3n@gitlab.com/g/s/r/", "https://gitlab.com/g/s/r");
        ok("ssh://git@host.io:2222/o/r.git", "https://host.io/o/r");
        ok("git://host.io/o/r", "https://host.io/o/r");
        ok("http://intranet:8080/o/r.git", "http://intranet:8080/o/r");
        for bad in ["", "/srv/git/r.git", "../r", r"C:\repos\r", "file:///r", "ext::sh -c id", "https://host", "-oProxyCommand=x:y/z"] {
            assert_eq!(web_url(bad), None, "{bad}");
        }
    }

    #[test]
    fn picks_origin_before_other_remotes() {
        let cfg = "remote.up.url git@a.com:x/up.git\nremote.origin.url https://b.com/x/me.git\nremote.loc.url /srv/r\n";
        assert_eq!(pick_remote(cfg).as_deref(), Some("https://b.com/x/me"));
        assert_eq!(pick_remote("remote.loc.url /srv/r\nremote.up.url git@a.com:x/up\n").as_deref(), Some("https://a.com/x/up"));
        assert_eq!(pick_remote(""), None);
    }

    #[test]
    fn numstat_sums_text_and_skips_binaries() {
        assert_eq!(parse_numstat("10\t2\tsrc/a.rs\n-\t-\timg.png\n3\t0\tb.md\n"), (13, 2));
        assert_eq!(parse_numstat(""), (0, 0));
    }

    #[test]
    fn reads_a_real_repo_with_worktree() {
        if Command::new("git").arg("--version").output().is_err() {
            return; // sem git na máquina: o recurso simplesmente não aparece
        }
        let root = std::env::temp_dir().join(format!("arbor-git-{}", std::process::id()));
        let (main, wt) = (root.join("main"), root.join("wt"));
        std::fs::create_dir_all(&main).unwrap();
        let run = |dir: &Path, args: &[&str]| {
            let ok = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap().status.success();
            assert!(ok, "git {args:?}");
        };
        run(&main, &["init", "-q", "-b", "main"]);
        run(&main, &["remote", "add", "origin", "git@github.com:o/r.git"]);
        std::fs::write(main.join("a.txt"), "um\n").unwrap();
        run(&main, &["add", "."]);
        run(&main, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "init"]);
        std::fs::write(main.join("a.txt"), "um\ndois\n").unwrap();
        std::fs::write(main.join("b.txt"), "novo\n").unwrap();
        let info = read(&main).expect("repositório");
        assert_eq!(info.branch, "main");
        assert_eq!(info.worktree, None);
        assert_eq!(info.remote.as_deref(), Some("https://github.com/o/r"));
        assert_eq!((info.unstaged, info.untracked, info.added_lines), (1, 1, 1));
        run(&main, &["worktree", "add", "-q", "-b", "feat", wt.to_str().unwrap()]);
        let linked = read(&wt).expect("worktree");
        std::fs::remove_dir_all(&root).ok();
        assert_eq!(linked.branch, "feat");
        assert_eq!(linked.worktree.as_deref(), Some("wt"));
    }
}
