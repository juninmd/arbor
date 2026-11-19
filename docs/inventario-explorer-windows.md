# arbor × Explorer do Windows 11: o que falta

_19/11/2026, sobre a `main` v0.2.0 + "Este Computador" (este PR). Complementa o inventário dos 22 explorers open source (`pesquisa/inventario-features-concorrentes.md`, fora do repo)._

**Prioridade:** P0 = sem isso, quem vem do Explorer estranha no primeiro minuto · P1 = uso diário · P2 = completa a paridade · P3 = nicho.
**Estado:** ✅ tem · 🟡 em parte · ❌ não tem.

## Navegação

| Feature do Explorer | arbor | Prio |
|---|---|---|
| Abrir em "Este Computador" com pastas do usuário e unidades | ✅ (este PR) | P0 |
| Unidades com barra de uso (vermelha quando quase cheia) e "X livres de Y" | ✅ (este PR) | P0 |
| Breadcrumb começando em "Este Computador"; subir da raiz volta para ele | ✅ (este PR) | P0 |
| Breadcrumb com seta `›` que abre um menu das subpastas | ❌ | P1 |
| Barra de endereço editável ao clicar, com autocomplete de caminho | ✅ (Ctrl+L ou duplo clique; Tab completa, setas escolhem) | P0 |
| Caixa "Pesquisar em <pasta>" com busca recursiva em subpastas | ❌ (só filtro da pasta) | P0 |
| Abas (Ctrl+T/W), arrastar aba para virar janela | 🟡 (abas sim, destacar não) | P2 |
| Voltar/avançar com histórico em menu (clique longo) e botões 4/5 do mouse | 🟡 (Alt+←/→ e botões do mouse; sem menu de histórico) | P1 |
| Árvore de pastas expansível na barra lateral | ❌ | P1 |
| "Início" com Recentes, Favoritos e Acesso rápido | ❌ | P1 |
| Fixar pastas na barra lateral (arrastar para fixar, desafixar) | 🟡 (pelo menu; arrastar não) | P0 |
| Galeria (fotos de todas as pastas de imagens por data) | ❌ | P3 |
| OneDrive/nuvem com estado de sincronização | ❌ (fora do escopo, ver plano) | — |
| Locais de rede, compartilhamentos SMB, mapear unidade | ❌ | P2 |
| Lixeira como local navegável (restaurar, esvaziar) | ❌ | P1 |
| Nova janela (Ctrl+N), restaurar janelas/abas ao abrir | 🟡 (Ctrl+N sim; restaurar sessão é da outra frente) | P1 |

## Barra de comandos e menu de contexto

| Feature do Explorer | arbor | Prio |
|---|---|---|
| Barra de comandos visível: Novo, Recortar, Copiar, Colar, Renomear, Compartilhar, Excluir, Classificar, Visualizar, "…" | ✅ (sem Compartilhar) | P0 |
| Menu de contexto (botão direito) em item e em área vazia | ✅ (também Shift+F10 e tecla Menu) | P0 |
| "Novo" com tipos (pasta, atalho, documento de texto, .zip…) | 🟡 (pasta e documento de texto) | P1 |
| "Abrir com" (lista de apps + escolher outro) | 🟡 (só app padrão) | P1 |
| Propriedades (tamanho em disco, datas, atributos, permissões, hash) | 🟡 (Alt+Enter abre no painel de detalhes; sem hash e sem editar) | P1 |
| Compactar em ZIP / extrair tudo; navegar dentro de .zip | ❌ (fase 3) | P1 |
| Copiar como caminho | ✅ | — |
| Abrir no Terminal | ✅ | — |
| Compartilhar (painel do sistema) | ❌ | P3 |
| Definir como papel de parede, girar imagem | ❌ | P3 |
| Itens de terceiros no menu (7-Zip, VS Code) — shell extensions | ❌ | P2 |

## Visualização

| Feature do Explorer | arbor | Prio |
|---|---|---|
| Detalhes e miniaturas | ✅ | — |
| Ícones extra grandes/grandes/médios/pequenos, Lista, Blocos, Conteúdo | 🟡 (detalhes + 4 tamanhos de ícone; sem Blocos/Conteúdo) | P1 |
| Ctrl+roda muda o tamanho dos ícones | ✅ (também Ctrl+= / Ctrl+-) | P1 |
| Colunas configuráveis (adicionar, reordenar, redimensionar por arraste) | ❌ | P1 |
| Agrupar por (tipo, data "Hoje/Ontem/Semana passada", tamanho) | ❌ | P2 |
| Filtro pelo cabeçalho da coluna (por tipo, por data) | ❌ | P2 |
| Cada pasta lembra visão e ordenação | ❌ | P1 |
| Painel de detalhes à direita (Alt+Shift+P) | ✅ | P1 |
| Painel de visualização à direita (Alt+P) | 🟡 (preview em sobreposição, Espaço) | P1 |
| Miniaturas de vídeo, PDF, Office | 🟡 (só imagem) | P2 |
| Ícones reais dos programas (.exe, .lnk) e da associação | ❌ | P1 |
| Mostrar extensões, itens ocultos, caixas de seleção | 🟡 (ocultos sim) | P2 |
| Modo compacto (densidade) | ❌ | P3 |

## Seleção e operações

| Feature do Explorer | arbor | Prio |
|---|---|---|
| Copiar, mover, renomear, lixeira, nova pasta, desfazer | ✅ | — |
| Seleção por retângulo (arrastar com o mouse) | ✅ (Ctrl soma à seleção) | P0 |
| Ctrl/Shift+clique, Ctrl+A, inverter seleção | ✅ | P2 |
| Arrastar e soltar dentro do app e para outros apps | 🟡 (dentro do app: pastas, barra lateral, outro painel; arrastar para outros apps não) | P0 |
| Arrastar com botão direito (copiar/mover/criar atalho aqui) | ❌ | P2 |
| Diálogo de conflito: substituir, pular, comparar, manter os dois | 🟡 (sempre "manter os dois" com nome único; sem perguntar) | P0 |
| Janela de progresso com gráfico de velocidade, pausa | 🟡 (barra + cancelar) | P1 |
| Refazer (Ctrl+Y) | ❌ | P1 |
| Excluir permanentemente (Shift+Del) com confirmação | ❌ | P1 |
| Renomear com F2 selecionando só o nome sem extensão; Tab vai ao próximo | 🟡 (prompt, não inline) | P1 |
| Renomear vários: "foto (1)", "foto (2)" | ❌ | P2 |
| Criar atalho / link simbólico | ❌ | P2 |
| Área de transferência do sistema (copiar no arbor, colar no Explorer e vice-versa) | ✅ (recortar vira mover só dentro do arbor) | P0 |
| Executar como administrador; pedir elevação ao copiar em pasta protegida | ❌ | P2 |

## Unidades e sistema

| Feature do Explorer | arbor | Prio |
|---|---|---|
| Nome do volume ("Disco Local", "Backup") e ícone por tipo (SSD, USB, DVD, rede) | 🟡 (letra/ponto de montagem) | P1 |
| Ejetar unidade removível | ❌ | P1 |
| Atualizar a lista quando um pendrive entra ou sai | ❌ (F5 em Este Computador) | P1 |
| Formatar, BitLocker, propriedades da unidade | ❌ | P3 |
| Gerenciador de arquivos padrão (abrir pastas do sistema no arbor), instância única | ❌ (fase 3) | P1 |
| Teclado: Alt+D, F4, F6 ciclo de áreas, Alt+Enter, Ctrl+Shift+N, letras pulam para o item | 🟡 (Alt+D, F4, Alt+Enter e Ctrl+Shift+N sim; digitar filtra em vez de pular; sem F6) | P1 |
| Leitor de tela e navegação só por teclado em todas as áreas | 🟡 | P1 |

## Ordem sugerida

1. **P0 de interação**: menu de contexto, barra de comandos, seleção por retângulo, arrastar e soltar interno, fixar pastas, busca recursiva, autocomplete no endereço, diálogo de conflito e área de transferência do sistema.
2. **P1 de visual**: painel de detalhes/preview à direita, árvore lateral, tamanhos de ícone com Ctrl+roda, colunas configuráveis, ícones reais, nome de volume.
3. **P1 de poder**: Propriedades, Abrir com, ZIP, Lixeira navegável, refazer, Shift+Del, restaurar sessão, instância única.
4. P2/P3 conforme pedido.
