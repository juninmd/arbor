#!/usr/bin/env bash
# Compara o arbor com as ferramentas que todo mundo usa para listar e filtrar pastas grandes.
# Mesma pasta (100 mil arquivos gerados pelo arbor-bench), mesma máquina, hyperfine com aquecimento.
#
#   scripts/bench-compare.sh [pasta] [saida.md]
#
# Tudo que estiver instalado entra: ls, eza, find, fd, fzf (Linux/macOS); dir e Get-ChildItem (Windows).
set -euo pipefail

DIR="${1:-bench-100k}"
OUT="${2:-bench-compare.md}"
BIN="target/release/arbor-bench"
[[ -x "$BIN" || -x "$BIN.exe" ]] || cargo build --release -p arbor-bench
[[ -x "$BIN.exe" ]] && BIN="$BIN.exe"
"$BIN" gen "$DIR" 100000

have() { command -v "$1" >/dev/null 2>&1; }

# hyperfine no Windows chama shells personalizados com "/C"; lá os comandos vão embrulhados em `bash -c`, sem shell.
SHELL_OPT=(--shell bash)
if [[ "${OS:-}" == "Windows_NT" ]]; then SHELL_OPT=(-N); fi
wrap() {
  if [[ "${OS:-}" == "Windows_NT" ]]; then printf 'bash -c %q' "$1"; else printf '%s' "$1"; fi
}
FD=""; have fd && FD=fd; have fdfind && FD=fdfind

list=(-n "arbor (listar + stat + ordem natural)" "$BIN list $DIR")
if have ls && ls --sort=version / >/dev/null 2>&1; then
  list+=(-n "ls -l --sort=version (GNU)" "ls -l --sort=version $DIR")
elif have ls; then
  list+=(-n "ls -l" "ls -l $DIR")
fi
have eza && list+=(-n "eza -l" "eza -l $DIR")
if have find && find "$DIR" -maxdepth 0 -printf '' >/dev/null 2>&1; then
  list+=(-n "find -printf tamanho+data (sem ordenar)" "find $DIR -maxdepth 1 -printf '%s %T@ %p\n'")
fi
[[ -n "$FD" ]] && list+=(-n "fd (só nomes, sem stat)" "$FD --max-depth 1 . $DIR")
if [[ "${OS:-}" == "Windows_NT" ]]; then
  WDIR="$(cygpath -w "$DIR" 2>/dev/null || echo "$DIR")"
  list+=(-n "cmd dir" "cmd //c dir /-C \"$WDIR\"")
  have pwsh && list+=(-n "PowerShell Get-ChildItem" "pwsh -NoProfile -Command \"Get-ChildItem -LiteralPath '$WDIR' | Sort-Object Name | Out-Null\"")
fi

for i in "${!list[@]}"; do (( i % 3 == 2 )) && list[i]="$(wrap "${list[i]}")"; done

{
  echo "### Listar uma pasta com 100 mil arquivos ($(uname -s 2>/dev/null || echo Windows), $(nproc 2>/dev/null || echo ?) núcleos)"
  echo
} > "$OUT"
hyperfine --warmup 3 --runs 10 --style basic "${SHELL_OPT[@]}" --export-markdown list.md "${list[@]}"
cat list.md >> "$OUT"

# Filtro fuzzy: os mesmos 100 mil nomes, a mesma consulta, saída descartada.
"$BIN" list "$DIR" >/dev/null
( cd "$DIR" && ls ) > names.txt 2>/dev/null || "$BIN" list "$DIR" > names.txt
filter=(-n "arbor filter (nucleo, paralelo)" "$BIN filter vislo < names.txt")
have fzf && filter+=(-n "fzf --filter" "fzf --filter vislo < names.txt")
for i in "${!filter[@]}"; do (( i % 3 == 2 )) && filter[i]="$(wrap "${filter[i]}")"; done
hyperfine --warmup 3 --runs 10 --style basic "${SHELL_OPT[@]}" --export-markdown filter.md "${filter[@]}"
{
  echo
  echo "### Filtrar 100 mil nomes com a consulta \"vislo\" (processo inteiro, inclui ler a entrada)"
  echo
  cat filter.md
} >> "$OUT"
rm -f list.md filter.md names.txt
cat "$OUT"
