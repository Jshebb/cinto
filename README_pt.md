# Cinto (Português)

Cinto é uma interface de usuário em terminal escrita em Rust, projetada para experimentar com agentes de codificação locais e de código aberto (como `gpt-oss`) dentro de um ambiente controlado.

## 🤖 Propósito Central
O Cinto atua como um *harness* que gerencia a interação entre um modelo de linguagem grande (LLM) rodando localmente (via endpoint compatível com OpenAI) e um sistema de arquivos/workspace local. Seu objetivo é permitir que os desenvolvedores testem as capacidades dos agentes — como ler arquivos, pesquisar código e fazer edições — de maneira estruturada e observável.

## ✨ Principais Recursos
*   **Interface TUI (Terminal UI):** Fornece uma interface dedicada para chat, gerenciamento de configurações e visualização de sugestões de workspace.
*   **Loop do Agente:** Gerencia o formato da conversa (suportando especificamente **Harmony** e **OpenAI tools**) para garantir que os servidores locais possam interagir com o estado do agente de forma confiável.
*   **Ferramentas de Workspace:** Expõe operações de sistema de arquivos de leitura aos LLMs através de funções como `list_files`, `read_file`, `write_file`, `delete_file` e `search`.
*   **Gerenciamento de Estado:** Inclui uma lista de tarefas (todo list) em memória (`todo_read`/`todo_write`) que o agente pode rastrear.
*   **Compressão Automática de Contexto:** Compacta resultados grandes de ferramentas e, quando o prompt se aproxima da janela configurada, troca o histórico antigo por um resumo marcado com `<CINTO_CONTEXT_COMPACTED>`.
*   **Segurança e Debugging:** Recursos como `/diff` (para ver mudanças), `/checkpoint [label]` (para salvar snapshots não destrutivos de patches) e comandos explícitos de inspeção de prompts/ferramentas são integrados para segurança.

## 📦 Instalação
Depois de criar uma tag `v*` e publicar os binários pelo workflow de release:

```sh
curl -fsSL https://raw.githubusercontent.com/joaoh/cinto/main/install.sh | sh
```

Usuários Node também poderão instalar pelo pacote npm:

```sh
npm install -g cinto
npx cinto
```

## ⚙️ Detalhes Técnicos
*   **Linguagem:** Rust (`cargo run`).
*   **Configuração:** Utiliza um arquivo `config.toml` para definir o endpoint do LLM, nome do modelo, formato (ex: `harmony`, `openai-tools`), temperatura, janela de contexto e limites de compressão.
*   **Compatibilidade de Modelo:** Suporta múltiplos formatos de prompt, adaptando sua comunicação com base se o servidor alvo espera o formato especializado **Harmony** ou a estrutura padrão **OpenAI tools**.
*   **Instruções de Workspace:** Lê `AGENTS.md` na raiz do workspace e injeta esse conteúdo nas instruções do modelo.

## 🚀 Como Começar
1.  Inicie um servidor LLM local que exponha um endpoint `/v1/completions` compatível com OpenAI (por exemplo, usando LM Studio).
2.  Execute `cinto`.
3.  Na primeira execução, o Cinto abre um setup TUI com a logo grande. Escolha o preset do servidor, confirme endpoint/model/workspace e salve.

Você pode reabrir o setup com:

```sh
cinto setup
```

Ou dentro do TUI:

```text
/setup
```

Em resumo, é uma estrutura robusta para construir e testar assistentes de codificação IA localmente!
