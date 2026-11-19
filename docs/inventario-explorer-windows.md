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
| Breadcrumb com seta `›` que abre um menu das subpastas | ✅ (em "Este Computador" lista pastas e discos) | P1 |
| Barra de endereço editável ao clicar, com autocomplete de caminho | ✅ (Ctrl+L ou duplo clique; Tab completa, setas escolhem) | P0 |
| Caixa "Pesquisar em <pasta>" com busca recursiva em subpastas | ✅ (Ctrl+Shift+F abre uma aba de resultados) | P0 |
| Abas (Ctrl+T/W), arrastar aba para virar janela | 🟡 (abas sim, destacar não) | P2 |
| Voltar/avançar com histórico em menu (clique longo) e botões 4/5 do mouse | ✅ (Alt+←/→, botões do mouse; botão direito em Voltar/Avançar mostra o histórico) | P1 |
| Árvore de pastas expansível na barra lateral | ✅ (locais, fixados e discos; até 500 subpastas por nó) | P1 |
| "Início" com Recentes, Favoritos e Acesso rápido | ❌ | P1 |
| Fixar pastas na barra lateral (arrastar para fixar, desafixar) | 🟡 (pelo menu; arrastar não) | P0 |
| Galeria (fotos de todas as pastas de imagens por data) | ❌ | P3 |
| OneDrive/nuvem com estado de sincronização | ❌ (fora do escopo, ver plano) | — |
| Locais de rede, compartilhamentos SMB, mapear unidade | 🟡 (`\\servidor` na barra de endereço lista os compartilhamentos no Windows; "Mapear unidade de rede…" em Este Computador; montagens smb/sftp do GVFS aparecem como unidades de rede no Linux; sem descoberta automática de computadores na rede) | P2 |
| Lixeira como local navegável (restaurar, esvaziar) | ✅ (Windows e Linux; no macOS o sistema não deixa listar) | P1 |
| Nova janela (Ctrl+N), restaurar janelas/abas ao abrir | ✅ (Ctrl+N; restaurar abas ao abrir, liga/desliga pela paleta) | P1 |

## Barra de comandos e menu de contexto

| Feature do Explorer | arbor | Prio |
|---|---|---|
| Barra de comandos visível: Novo, Recortar, Copiar, Colar, Renomear, Compartilhar, Excluir, Classificar, Visualizar, "…" | ✅ (sem Compartilhar) | P0 |
| Menu de contexto (botão direito) em item e em área vazia | ✅ (também Shift+F10 e tecla Menu) | P0 |
| "Novo" com tipos (pasta, atalho, documento de texto, .zip…) | 🟡 (pasta e documento de texto) | P1 |
| "Abrir com" (lista de apps + escolher outro) | 🟡 (Windows: diálogo nativo; Linux/macOS: digitar o programa) | P1 |
| Propriedades (tamanho em disco, datas, atributos, permissões, hash) | 🟡 (Alt+Enter abre no painel de detalhes, com SHA-256 sob demanda; sem editar atributos) | P1 |
| Compactar em ZIP / extrair tudo; navegar dentro de .zip | ✅ (compactar em zip, extrair zip/7z/tar, abrir o compactado como pasta) | P1 |
| Copiar como caminho | ✅ | — |
| Abrir no Terminal | ✅ | — |
| Compartilhar (painel do sistema) | ❌ | P3 |
| Definir como papel de parede, girar imagem | ❌ | P3 |
| Itens de terceiros no menu (7-Zip, VS Code) — shell extensions | ❌ | P2 |

## Visualização

| Feature do Explorer | arbor | Prio |
|---|---|---|
| Detalhes e miniaturas | ✅ | — |
| Ícones extra grandes/grandes/médios/pequenos, Lista, Blocos, Conteúdo | ✅ (Conteúdo, Detalhes, Lista, Blocos e 4 tamanhos de ícone, na ordem do Ctrl+roda) | P1 |
| Ctrl+roda percorre Detalhes → Lista → Blocos → ícones P/M/G/XG | ✅ (também Ctrl+= / Ctrl+-; o modo aparece na barra de status) | P1 |
| Colunas configuráveis (adicionar, mostrar/ocultar, redimensionar por arraste) | 🟡 (mostrar/ocultar pelo botão direito no cabeçalho e largura por arraste; reordenar colunas não implementado) | P1 |
| Agrupar por (tipo, data "Hoje/Ontem/Semana passada", tamanho) | 🟡 (Classificar ou botão direito → Agrupar por; cabeçalhos com contagem em Detalhes; na grade só ordena por grupo, sem cabeçalho, e não fica salvo por pasta) | P2 |
| Filtro pelo cabeçalho da coluna (por tipo, por data) | ❌ | P2 |
| Cada pasta lembra visão e ordenação | ✅ (`~/.config/arbor/views`) | P1 |
| Painel de detalhes à direita (Alt+Shift+P) | ✅ | P1 |
| Painel de visualização à direita (Alt+P) | 🟡 (preview em sobreposição, Espaço) | P1 |
| Miniaturas de vídeo, PDF, Office | 🟡 (imagem, capa de áudio, vídeo e PDF com ffmpeg/pdftoppm ou Quick Look no macOS; Office não) | P2 |
| Ícones reais dos programas (.exe, .lnk) e da associação | 🟡 (Windows: ícone real de .exe, .lnk, .msi e .url nas visualizações em grade e Blocos; em Detalhes e na Lista, e o ícone do programa associado a cada tipo, ainda não) | P1 |
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
| Diálogo de conflito: substituir, pular, comparar, manter os dois | 🟡 (pergunta Substituir, Pular ou Manter os dois quando há nome repetido; sem "comparar") | P0 |
| Janela de progresso com gráfico de velocidade, pausa | 🟡 (barra + cancelar) | P1 |
| Refazer (Ctrl+Y) | ✅ | P1 |
| Excluir permanentemente (Shift+Del) com confirmação | ✅ | P1 |
| Renomear com F2 selecionando só o nome sem extensão; Tab vai ao próximo | 🟡 (prompt, não inline) | P1 |
| Renomear vários: "foto (1)", "foto (2)" | ❌ | P2 |
| Criar atalho / link simbólico | ❌ | P2 |
| Área de transferência do sistema (copiar no arbor, colar no Explorer e vice-versa) | ✅ (recortar vira mover só dentro do arbor) | P0 |
| Executar como administrador; pedir elevação ao copiar em pasta protegida | ❌ | P2 |

## Unidades e sistema

| Feature do Explorer | arbor | Prio |
|---|---|---|
| Nome do volume ("Disco Local", "Backup") e ícone por tipo (SSD, USB, DVD, rede) | ✅ (nome do volume no Windows; ícones distintos para disco do sistema, disco local, USB/cartão, DVD e rede, na lista e na barra lateral) | P1 |
| Ejetar unidade removível | ✅ (menu do disco em Este Computador) | P1 |
| Atualizar a lista quando um pendrive entra ou sai | ✅ (varredura a cada 4 s) | P1 |
| Formatar, BitLocker, propriedades da unidade | ❌ | P3 |
| Gerenciador de arquivos padrão (abrir pastas do sistema no arbor), instância única | ✅ (paleta: "Usar o arbor para abrir pastas"; segunda execução vira aba na janela aberta) | P1 |
| Teclado: Alt+D, F4, F6 ciclo de áreas, Alt+Enter, Ctrl+Shift+N, letras pulam para o item | 🟡 (Alt+D, F4, Alt+Enter e Ctrl+Shift+N sim; digitar filtra em vez de pular; sem F6) | P1 |
| Leitor de tela e navegação só por teclado em todas as áreas | 🟡 | P1 |

## Ordem sugerida

1. **P0 de interação**: menu de contexto, barra de comandos, seleção por retângulo, arrastar e soltar interno, fixar pastas, busca recursiva, autocomplete no endereço, diálogo de conflito e área de transferência do sistema.
2. **P1 de visual**: painel de detalhes/preview à direita, árvore lateral, tamanhos de ícone com Ctrl+roda, colunas configuráveis, ícones reais, nome de volume.
3. **P1 de poder**: Propriedades, Abrir com, ZIP, Lixeira navegável, refazer, Shift+Del, restaurar sessão, instância única.
4. P2/P3 conforme pedido.
