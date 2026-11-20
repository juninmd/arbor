//! Branch do git da pasta aberta, para a barra de status (Files, One Commander, o plugin do Dolphin). Só lê
//! arquivos pequenos (`.git/HEAD`), sem rodar o `git`: sobe da pasta até achar o `.git`.

use std::path::{Path, PathBuf};

/// Diretório git de `dir` ou de uma pasta acima: `.git` pasta, ou arquivo `gitdir: ...` (worktree, submódulo).
fn git_dir(dir: &Path) -> Option<PathBuf> {
    for d in dir.ancestors().take(64) {
        let dot = d.join(".git");
        let Ok(meta) = std::fs::metadata(&dot) else { continue };
        if meta.is_dir() {
            return Some(dot);
        }
        let text = std::fs::read_to_string(&dot).ok()?;
        let target = PathBuf::from(text.trim().strip_prefix("gitdir:")?.trim());
        return Some(if target.is_relative() { d.join(target) } else { target });
    }
    None
}

/// Nome da branch, ou o commit curto quando o HEAD está solto. `None` fora de repositório.
pub fn branch_of(dir: &Path) -> Option<String> {
    let head = std::fs::read_to_string(git_dir(dir)?.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref:") {
        Some(r) => Some(r.trim().trim_start_matches("refs/heads/").to_string()),
        None => (head.len() >= 7).then(|| head[..7].to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::branch_of;
    use std::fs;

    #[test]
    fn branch_em_subpasta_worktree_e_head_solto() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("src/deep")).unwrap();
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/feat/x\n").unwrap();
        assert_eq!(branch_of(&repo.join("src/deep")).as_deref(), Some("feat/x"));
        let wt = d.path().join("wt");
        fs::create_dir_all(repo.join(".git/worktrees/wt")).unwrap();
        fs::write(repo.join(".git/worktrees/wt/HEAD"), "0123456789abcdef\n").unwrap();
        fs::create_dir_all(&wt).unwrap();
        fs::write(wt.join(".git"), "gitdir: ../repo/.git/worktrees/wt\n").unwrap();
        assert_eq!(branch_of(&wt).as_deref(), Some("0123456"));
        assert_eq!(branch_of(d.path()), None);
    }
}
