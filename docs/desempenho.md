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
