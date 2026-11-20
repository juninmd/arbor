#!/usr/bin/env bash
# Mede o arbor de verdade sob Xvfb: miniaturas (frio e com cache), CPU e memória.
# uso: scripts/bench-gui.sh <pasta-com-imagens> [segundos]   (precisa de Xvfb; ARBOR_BIN muda o binário)
set -eu
dir=$(realpath "${1:?pasta com imagens}"); secs=${2:-12}
bin=${ARBOR_BIN:-target/release/arbor}
work=$(mktemp -d); trap 'kill $xp $ap 2>/dev/null || true; rm -rf "$work"' EXIT
export HOME=$work/home XDG_CONFIG_HOME=$work/cfg XDG_CACHE_HOME=$work/cache DISPLAY=:97 ARBOR_NO_GLASS=1 ARBOR_NO_UPDATE=1 ARBOR_NO_ANIM=1
mkdir -p "$HOME" "$XDG_CONFIG_HOME/arbor" "$XDG_CACHE_HOME"
printf 'restore_session=true\n' > "$XDG_CONFIG_HOME/arbor/settings"
printf '{"dual":false,"active":0,"panes":[{"active":0,"tabs":[{"path":"%s","grid":true}]},{"active":0,"tabs":[{"path":"%s","grid":false}]}]}' "$dir" "$HOME" > "$XDG_CONFIG_HOME/arbor/session.json"
Xvfb :97 -screen 0 1320x840x24 >/dev/null 2>&1 & xp=$!
sleep 1
hz=$(getconf CLK_TCK)
run() { # $1 = rótulo
  "$bin" >/dev/null 2>&1 & ap=$!
  local t0=$(date +%s.%N) first="" n last=0 doneat=0
  for ((i = 0; i < secs * 10; i++)); do
    sleep 0.1
    n=$(find "$XDG_CACHE_HOME/arbor/thumbs" -type f 2>/dev/null | wc -l)
    if [[ $n -ne $last ]]; then last=$n; doneat=$(echo "$(date +%s.%N) - $t0" | bc); fi
    if [[ -z $first && $n -gt 0 ]]; then first=$(echo "$(date +%s.%N) - $t0" | bc); fi
  done
  read -r _ _ _ _ _ _ _ _ _ _ _ _ _ ut st _ < /proc/$ap/stat
  local rss=$(awk '/VmHWM/{print int($2/1024)}' /proc/$ap/status)
  echo "$1 | 1ª miniatura em cache: ${first:-n/d} s | última em: ${doneat} s | miniaturas: $n | CPU: $(echo "scale=2; ($ut+$st)/$hz" | bc) s | pico de memória: ${rss} MB"
  kill $ap; wait $ap 2>/dev/null || true
}
scroll() { # rolagem rápida com o cache vazio: quantas miniaturas inúteis são geradas
  rm -rf "$XDG_CACHE_HOME/arbor"
  "$bin" >/dev/null 2>&1 & ap=$!
  sleep 4
  local before; before=$(find "$XDG_CACHE_HOME/arbor/thumbs" -type f 2>/dev/null | wc -l)
  local t0=$(date +%s.%N)
  python3 "$(dirname "$0")/scroll.py" 300 5
  local last=$before n quiet=0
  while ((quiet < 20)); do
    sleep 0.1; n=$(find "$XDG_CACHE_HOME/arbor/thumbs" -type f | wc -l)
    if [[ $n -ne $last ]]; then last=$n; quiet=0; else quiet=$((quiet + 1)); fi
  done
  echo "rolagem rápida (300 passos em ~1,5 s) | na 1ª tela: $before | geradas durante a rolagem: $((last - before)) | até a fila esvaziar: $(echo "$(date +%s.%N) - $t0 - 2" | bc) s"
  kill $ap; wait $ap 2>/dev/null || true
}
echo "| cenário | resultado |"; run "frio"; run "com cache"; scroll
