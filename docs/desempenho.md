# Desempenho

Medidas reproduzíveis (máquina de 4 núcleos, Linux, Xvfb com renderizador por software: valem para comparar
antes/depois, não como número absoluto de quadros).

| Comando | O que mede |
|---|---|
| `cargo run -p arbor-bench --release -- suite <pasta> --check` | primeiro quadro, listagem, ordenação, filtro, memória (100 mil itens) |
| `cargo run -p arbor-bench --release -- churn <pasta> [s]` | pasta que muda sem parar: leituras e CPU, política antiga × atual |
| `scripts/bench-gui.sh <pasta-com-fotos>` | app real: miniaturas a frio e com cache, CPU, memória, rolagem rápida (`ARBOR_BIN` troca o binário; precisa de Xvfb e `python-xlib`) |

## Resultados (PR de desempenho 1)

| Cenário | Antes | Depois |
|---|---:|---:|
| 1ª tela de 1500 fotos, cache vazio: miniaturas geradas | 89 | 49 (só as visíveis) |
| Rolagem rápida (300 passos): miniaturas geradas além da 1ª tela | 136 | 59 |
| Pasta de 100 mil itens com escrita contínua (8 s): leituras | 21 | 12 |
| Idem: CPU | 84 % de 1 núcleo | 62 % |

Sob Xvfb não há GPU: o arbor cai no renderizador `tiny-skia`, que domina o CPU da thread da interface
(gradiente e imagem esticada da aurora). Medir quadros de verdade exige uma GPU.

## Duplicados (PR de desempenho 2)

`arbor-bench dupes <pasta>`: 800 conteúdos × 2 cópias de ~400 KB, cache quente, 4 núcleos, mediana de 9 rodadas.
Grupos de 2 arquivos agora são comparados direto (cada arquivo lido uma vez) em vez de hash dos dois e depois comparação.

| | Antes | Depois |
|---|---:|---:|
| Busca de duplicados | 300 ms | 120 ms |

Testado e descartado: paralelizar entre grupos de tamanho (125 → 120 ms, dentro do ruído com cache quente).

### Medidos e sem ganho a buscar (100 mil arquivos, cache quente, 4 núcleos, mediana de 9)

| Caminho | Resultado | Por quê parar |
|---|---:|---|
| Busca recursiva (`search`), 20 mil achados | 91–135 ms | ~1 µs por arquivo, variância (±25%) maior que qualquer ganho de alocação |
| Tamanho de pasta (`du::dir_size`) | 107–118 ms | `du -sb` leva 176 ms na mesma pasta |
