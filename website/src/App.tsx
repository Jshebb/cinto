import { motion, useScroll, useTransform } from "framer-motion";
import { useState, createContext, useContext, useEffect } from "react";
import type { ReactNode, SVGProps } from "react";
import terminalSetup from "./assets/terminal_setup.png";
import terminalUse from "./assets/terminal_use.png";
import { AnimatedGridPattern } from "@/components/ui/animated-grid-pattern";


// --- Types & Context ---
type Language = "en" | "pt";

const translations = {
  en: {
    nav: {
      features: "Features",
      stack: "Stack",
      workflow: "Workflow",
      crp: "CRP",
      install: "Install",
      github: "GitHub",
    },
    hero: {
      tag: "v0.1 · local agent harness",
      headline: ["The local-first", "agentic coding with a twist."],
      typewriter: [
        "agentic coding env.",
        "CRP reasoning layer.",
        "local model harness.",
        "approval-first agent.",
        "structured agent loop.",
      ],
      description: "A lightweight terminal workspace for coding agents, local models, OpenAI-style tools, Harmony-compatible workflows and approval-based repository edits.",
      cta_install: "Install Cinto",
      cta_github: "View on GitHub",
    },
    metrics: [
      ["16", "tools exposed"],
      ["CRP", "reasoning layer"],
      ["local", "model runtime"],
      ["100%", "open source"],
    ],
    terminal: {
      header: "~/projects/cinto · local endpoint",
      chat: "chat",
      ready: "SYS Ready",
      welcome: "Cinto is ready. Type a request, /tools, /todos, /prompt, /settings...",
      loop: "agent loop",
      steps: ["task interpretation", "relevant files", "proposed approach", "approval edits"],
      aside: {
        state: "ready",
        effort: "medium",
        format: "openai-tools",
        reasoning: "crp",
        tools: "16",
        edits: "approval",
      }
    },
    features: {
      tag: "Features",
      title: "A small harness for serious agent experiments.",
      description: "Cinto is built for the moment where local models are useful enough to code, but still need structure, visibility and guardrails.",
      items: [
        { title: "Agent terminal", text: "A focused TUI for coding agents with visible prompts, tools, todos, context and session state." },
        { title: "Compact-model first", text: "Designed to extract useful coding behavior from local models instead of assuming unlimited cloud inference." },
        { title: "Approval edits", text: "Agents can inspect and propose changes while you keep control over what actually touches the repository." },
        { title: "Reasoning protocol", text: "CRP-style structure separates task interpretation, relevant files, proposed approach and final response." },
      ]
    },
    workflow: {
      tag: "Workflow",
      title: "Keep the agent loop visible.",
      description: "Instead of hiding tool use behind a chat box, Cinto exposes the operational loop: context, tools, proposed plan, edits and final response.",
      steps: [
        ["01", "Connect", "Point Cinto at LM Studio, Ollama or any OpenAI-compatible endpoint."],
        ["02", "Inspect", "Expose the model loop: files, tools, prompt shape and context usage."],
        ["03", "Reason", "Use structured steps for planning instead of opaque one-shot chat."],
        ["04", "Approve", "Review proposed edits before applying changes to your repo."],
      ]
    },
    install: {
      tag: "Open source local agent harness",
      title: "Install Cinto and connect your model.",
      description: "Point it at your local endpoint, select a model and start testing agentic coding workflows with a visible control surface.",
      badges: ["LM Studio", "Ollama", "OpenAI-compatible", "Rust TUI"],
      terminal_label: "terminal",
      terminal_comment: "# inspect prompts, tools, todos and proposed edits",
      methods_title: "Choose an install path",
      methods: [
        {
          title: "Build from source",
          text: "Best for early builds and contributors. Requires Rust and Cargo.",
          command: "cargo install --git https://github.com/Jshebb/cinto",
        },
        {
          title: "Linux / macOS installer",
          text: "Downloads the release binary, verifies SHA-256 and installs it into your local bin directory.",
          command: "curl -fsSL https://raw.githubusercontent.com/Jshebb/cinto/main/install.sh | sh",
        },
        {
          title: "Windows PowerShell",
          text: "Installs the latest release into your user bin path and updates PATH for future terminals.",
          command: "Invoke-WebRequest -Uri https://raw.githubusercontent.com/Jshebb/cinto/main/install.ps1 -UseBasicParsing | Invoke-Expression",
        },
      ],
      quickstart_title: "First run",
      quickstart: [
        "Start LM Studio, Ollama or another OpenAI-compatible server.",
        "Run cinto and complete the guided setup screen.",
        "Choose endpoint, model, prompt format, workspace and edit policy.",
        "Ask a focused repository question and inspect /tools, /prompt and /diff.",
      ],
      model_title: "Recommended model formats",
      models: [
        ["LM Studio + gpt-oss", "http://127.0.0.1:1234", "harmony"],
        ["LM Studio + Qwen/Llama", "http://127.0.0.1:1234", "openai-tools"],
        ["Ollama + qwen2.5-coder", "http://127.0.0.1:11434", "openai-tools"],
      ],
      after_title: "Useful commands",
      commands: [
        ["cinto setup", "reopen the guided setup"],
        ["cinto --print-prompt", "inspect the rendered empty prompt"],
        ["cinto --config ./config.toml", "use a project-local config"],
        ["cinto uninstall --purge-config", "remove the binary and config"],
      ],
      safety: "Cinto is a local agent harness, not a sandbox. Read tools can send workspace contents to the configured endpoint, while file edits and deletes require approval by default.",
      terminal_title: "install",
      terminal_commands: [
        "curl -fsSL https://raw.githubusercontent.com/Jshebb/cinto/main/install.sh | sh",
        "cinto setup",
        "cinto",
      ],
    },
    walkthrough: {
      setup_tag: "01 — SETUP",
      setup_title: "Configure your local agent in one guided pass.",
      setup_description: "Cinto starts by mapping your workspace, local endpoint, model, tool format, reasoning mode and safety defaults — so you can go from first run to agentic coding without editing config files by hand.",
      setup_alt: "terminal setup",
      setup_items: [
        ["First-run setup", "Choose a preset, confirm your workspace and enter chat immediately."],
        ["Runtime presets", "LM Studio, Ollama, vLLM, OpenAI-compatible, minimal or fully custom."],
        ["Protocol-aware config", "Select OpenAI tools, Harmony-style workflows and CRP reasoning modes."],
        ["Safe by default", "Edit approval, shell tools and context compression are explicit choices."],
      ],
      use_tag: "02 — USE",
      use_title: "A coding agent that lives where you already work.",
      use_description: "Once configured, Cinto becomes a local coding assistant inside your terminal. Ask it to explore files, search patterns, explain code, plan changes and propose edits — while keeping the full session visible.",
      use_alt: "terminal use",
      use_items: [
        ["Repository-aware chat", "Ask questions about your current workspace without leaving the terminal."],
        ["Built-in coding commands", "Explore files, search code, view diffs, stage changes and manage checkpoints."],
        ["Live session state", "Track context usage, model effort, tool format, reasoning mode and edit policy."],
        ["Human-in-the-loop edits", "Let the agent propose changes while you stay in control of what gets applied."],
      ],
    },
    crp: {
      tag: "CRP — Cinto Reasoning Protocol",
      title: "A standardized reasoning loop for local coding agents.",
      description: "CRP gives small and local models a predictable scaffold: understand the task, identify relevant files, propose an approach, call tools safely and return a clean final answer.",
      code_title: "crp.turn",
      example: `TASK_INTERPRETATION
User wants to add a GitHub Pages landing page to the current repository.

RELEVANT_FILES
- package.json
- vite.config.ts
- src/App.tsx
- .github/workflows/deploy-pages.yml

PROPOSED_APPROACH
1. Create a Vite landing page.
2. Configure base path for GitHub Pages.
3. Add deployment workflow.
4. Keep generated edits approval-first.

TOOL_EXECUTION
read_file("package.json")
write_file("src/App.tsx")
write_file(".github/workflows/deploy-pages.yml")

FINAL_RESPONSE
Landing page added. Review the diff, then push to deploy.`,
    },
    stack: {
      title: "Built for local agent workflows.",
      description: "Cinto brings local models, tool-calling, structured reasoning and approval-based edits into one terminal-first coding environment.",
      disclaimer: "Compatible names refer to supported runtimes, model families, formats or API conventions. Cinto is not affiliated with or endorsed by those providers.",
      cards: [
        { eyebrow: "LOCAL", title: "Local LLMs", description: "Run through LM Studio, Ollama or any OpenAI-compatible endpoint. Your code stays on your machine.", icon: "◈" },
        { eyebrow: "TOOLS", title: "OpenAI Tools", description: "Use tool-calling workflows for file exploration, code search, diffs, staging and checkpoints.", icon: "⌘" },
        { eyebrow: "FORMAT", title: "Harmony", description: "Experiment with GPT-OSS style structured outputs and agent-friendly interaction formats.", icon: "H" },
        { eyebrow: "REASONING", title: "CRP", description: "Separate task interpretation, relevant files, proposed approach and final response.", icon: "C" },
        { eyebrow: "SETUP", title: "Runtime presets", description: "Start quickly with presets for LM Studio, Ollama, vLLM, OpenAI-compatible APIs or custom configs.", icon: "▣" },
        { eyebrow: "SEARCH", title: "Fast code search", description: "Let the agent inspect your workspace, find files and search patterns across the repository.", icon: "⌕" },
        { eyebrow: "SAFETY", title: "Approval diffs", description: "Review proposed changes before they touch disk. Keep the agent useful without giving up control.", icon: "✓" },
        { eyebrow: "CONTEXT", title: "Context control", description: "Track context usage, model effort, tool format and reasoning mode directly in the session UI.", icon: "◌" },
        { eyebrow: "WORKFLOW", title: "Checkpoints", description: "Stage, unstage, diff and checkpoint changes from the same terminal-first agent loop.", icon: "↻" },
      ],
    },
    code: {
      copy: "copy",
      copied: "copied",
    }
  },
  pt: {
    nav: {
      features: "Recursos",
      stack: "Tecnologias",
      workflow: "Fluxo",
      crp: "CRP",
      install: "Instalar",
      github: "GitHub",
    },
    hero: {
      tag: "v0.1 · harness de agentes local",
      headline: ["O harness local"],
      typewriter: [
        "para agentes de código.",
        "com raciocínio CRP.",
        "para LLMs.",
        "com edições aprovadas.",
        "com tool use visível.",
      ],
      description: "Um workspace de terminal leve para agentes de código, modelos locais, ferramentas estilo OpenAI, workflows compatíveis com Harmony e edições de repositório baseadas em aprovação.",
      cta_install: "Instalar Cinto",
      cta_github: "Ver no GitHub",
    },
    metrics: [
      ["16", "ferramentas"],
      ["CRP", "camada de raciocínio"],
      ["local", "runtime do modelo"],
      ["100%", "open source"],
    ],
    terminal: {
      header: "~/projetos/cinto · endpoint local",
      chat: "chat",
      ready: "SISTEMA Pronto",
      welcome: "Cinto está pronto. Digite um pedido, /tools, /todos, /prompt, /settings...",
      loop: "ciclo do agente",
      steps: ["interpretação da tarefa", "arquivos relevantes", "abordagem proposta", "aprovação de edições"],
      aside: {
        state: "pronto",
        effort: "médio",
        format: "openai-tools",
        reasoning: "crp",
        tools: "16",
        edits: "aprovação",
      }
    },
    features: {
      tag: "Recursos",
      title: "Um harness pequeno para experimentos sérios com agentes.",
      description: "Cinto foi construído para o momento em que modelos locais são úteis o suficiente para programar, mas ainda precisam de estrutura, visibilidade e guardrails.",
      items: [
        { title: "Terminal de agentes", text: "Uma TUI focada para agentes de código com prompts visíveis, ferramentas, tarefas, contexto e estado da sessão." },
        { title: "Compact-model first", text: "Projetado para extrair comportamento útil de modelos locais em vez de assumir inferência ilimitada na nuvem." },
        { title: "Aprovação de edições", text: "Agentes podem inspecionar e propor mudanças enquanto você mantém o controle sobre o que toca no repositório." },
        { title: "Protocolo de raciocínio", text: "Estrutura estilo CRP separa interpretação de tarefa, arquivos relevantes, abordagem proposta e resposta final." },
      ]
    },
    workflow: {
      tag: "Fluxo de Trabalho",
      title: "Mantenha o ciclo do agente visível.",
      description: "Em vez de esconder o uso de ferramentas em uma caixa de chat, o Cinto expõe o ciclo operacional: contexto, ferramentas, plano proposto, edições e resposta final.",
      steps: [
        ["01", "Conectar", "Aponte o Cinto para LM Studio, Ollama ou qualquer endpoint compatível com OpenAI."],
        ["02", "Inspecionar", "Exponha o ciclo do modelo: arquivos, ferramentas, formato de prompt e uso de contexto."],
        ["03", "Raciocinar", "Use passos estruturados para planejamento em vez de um chat opaco."],
        ["04", "Aprovar", "Revise edições propostas antes de aplicar mudanças ao seu repositório."],
      ]
    },
    install: {
      tag: "Harness de agentes local open source",
      title: "Instale o Cinto e conecte seu modelo.",
      description: "Aponte para seu endpoint local, selecione um modelo e comece a testar workflows de codificação agêntica com uma superfície de controle visível.",
      badges: ["LM Studio", "Ollama", "Compatível com OpenAI", "Rust TUI"],
      terminal_label: "terminal",
      terminal_comment: "# inspecione prompts, ferramentas, tarefas e edições propostas",
      methods_title: "Escolha uma forma de instalação",
      methods: [
        {
          title: "Compilar do código-fonte",
          text: "Melhor para builds iniciais e contribuidores. Requer Rust e Cargo.",
          command: "cargo install --git https://github.com/Jshebb/cinto",
        },
        {
          title: "Instalador Linux / macOS",
          text: "Baixa o binário de release, verifica SHA-256 e instala no seu diretório local de binários.",
          command: "curl -fsSL https://raw.githubusercontent.com/Jshebb/cinto/main/install.sh | sh",
        },
        {
          title: "Windows PowerShell",
          text: "Instala a última release no binário de usuário e atualiza o PATH para próximos terminais.",
          command: "Invoke-WebRequest -Uri https://raw.githubusercontent.com/Jshebb/cinto/main/install.ps1 -UseBasicParsing | Invoke-Expression",
        },
      ],
      quickstart_title: "Primeira execução",
      quickstart: [
        "Inicie LM Studio, Ollama ou outro servidor compatível com OpenAI.",
        "Rode cinto e complete a tela de setup guiado.",
        "Escolha endpoint, modelo, formato de prompt, workspace e política de edição.",
        "Faça uma pergunta focada sobre o repositório e inspecione /tools, /prompt e /diff.",
      ],
      model_title: "Formatos recomendados",
      models: [
        ["LM Studio + gpt-oss", "http://127.0.0.1:1234", "harmony"],
        ["LM Studio + Qwen/Llama", "http://127.0.0.1:1234", "openai-tools"],
        ["Ollama + qwen2.5-coder", "http://127.0.0.1:11434", "openai-tools"],
      ],
      after_title: "Comandos úteis",
      commands: [
        ["cinto setup", "reabre o setup guiado"],
        ["cinto --print-prompt", "inspeciona o prompt vazio renderizado"],
        ["cinto --config ./config.toml", "usa uma config local do projeto"],
        ["cinto uninstall --purge-config", "remove o binário e a configuração"],
      ],
      safety: "Cinto é um harness local de agente, não uma sandbox. Tools de leitura podem enviar conteúdo do workspace ao endpoint configurado, enquanto edições e deleções exigem aprovação por padrão.",
      terminal_title: "instalar",
      terminal_commands: [
        "curl -fsSL https://raw.githubusercontent.com/Jshebb/cinto/main/install.sh | sh",
        "cinto setup",
        "cinto",
      ],
    },
    walkthrough: {
      setup_tag: "01 — SETUP",
      setup_title: "Configure seu agente local em uma passagem guiada.",
      setup_description: "O Cinto começa mapeando workspace, endpoint local, modelo, formato de tools, modo de raciocínio e padrões de segurança — assim você sai da primeira execução direto para codificação agêntica sem editar arquivos de config manualmente.",
      setup_alt: "setup no terminal",
      setup_items: [
        ["Setup inicial", "Escolha um preset, confirme o workspace e entre no chat imediatamente."],
        ["Presets de runtime", "LM Studio, Ollama, vLLM, compatível com OpenAI, mínimo ou totalmente customizado."],
        ["Config sensível ao protocolo", "Selecione OpenAI tools, workflows estilo Harmony e modos de raciocínio CRP."],
        ["Seguro por padrão", "Aprovação de edição, shell tools e compressão de contexto são escolhas explícitas."],
      ],
      use_tag: "02 — USO",
      use_title: "Um agente de código que vive onde você já trabalha.",
      use_description: "Depois de configurado, o Cinto vira um assistente local de código dentro do seu terminal. Peça para explorar arquivos, buscar padrões, explicar código, planejar mudanças e propor edições — mantendo a sessão inteira visível.",
      use_alt: "uso no terminal",
      use_items: [
        ["Chat ciente do repositório", "Faça perguntas sobre o workspace atual sem sair do terminal."],
        ["Comandos de código integrados", "Explore arquivos, busque código, veja diffs, faça stage e gerencie checkpoints."],
        ["Estado vivo da sessão", "Acompanhe uso de contexto, esforço do modelo, formato de tools, modo de raciocínio e política de edição."],
        ["Edições com humano no loop", "Deixe o agente propor mudanças enquanto você controla o que é aplicado."],
      ],
    },
    crp: {
      tag: "CRP — Cinto Reasoning Protocol",
      title: "Um ciclo de raciocínio padronizado para agentes locais de código.",
      description: "CRP dá a modelos pequenos e locais uma estrutura previsível: entender a tarefa, identificar arquivos relevantes, propor uma abordagem, chamar tools com segurança e retornar uma resposta final limpa.",
      code_title: "crp.turn",
      example: `TASK_INTERPRETATION
O usuário quer adicionar uma landing page de GitHub Pages ao repositório atual.

RELEVANT_FILES
- package.json
- vite.config.ts
- src/App.tsx
- .github/workflows/deploy-pages.yml

PROPOSED_APPROACH
1. Criar uma landing page em Vite.
2. Configurar base path para GitHub Pages.
3. Adicionar workflow de deploy.
4. Manter edições geradas dependentes de aprovação.

TOOL_EXECUTION
read_file("package.json")
write_file("src/App.tsx")
write_file(".github/workflows/deploy-pages.yml")

FINAL_RESPONSE
Landing page adicionada. Revise o diff e depois envie para deploy.`,
    },
    stack: {
      title: "Construído para workflows de agentes locais.",
      description: "Cinto reúne modelos locais, tool-calling, raciocínio estruturado e edições baseadas em aprovação em um ambiente de código terminal-first.",
      disclaimer: "Nomes compatíveis se referem a runtimes, famílias de modelos, formatos ou convenções de API suportados. Cinto não é afiliado nem endossado por esses provedores.",
      cards: [
        { eyebrow: "LOCAL", title: "LLMs locais", description: "Rode via LM Studio, Ollama ou qualquer endpoint compatível com OpenAI. Seu código fica na sua máquina.", icon: "◈" },
        { eyebrow: "TOOLS", title: "OpenAI Tools", description: "Use workflows com tool-calling para explorar arquivos, buscar código, ver diffs, fazer stage e checkpoints.", icon: "⌘" },
        { eyebrow: "FORMATO", title: "Harmony", description: "Experimente outputs estruturados estilo GPT-OSS e formatos de interação amigáveis para agentes.", icon: "H" },
        { eyebrow: "RACIOCÍNIO", title: "CRP", description: "Separe interpretação da tarefa, arquivos relevantes, abordagem proposta e resposta final.", icon: "C" },
        { eyebrow: "SETUP", title: "Presets de runtime", description: "Comece rápido com presets para LM Studio, Ollama, vLLM, APIs compatíveis com OpenAI ou configs customizadas.", icon: "▣" },
        { eyebrow: "BUSCA", title: "Busca rápida no código", description: "Deixe o agente inspecionar o workspace, encontrar arquivos e buscar padrões pelo repositório.", icon: "⌕" },
        { eyebrow: "SEGURANÇA", title: "Diffs com aprovação", description: "Revise mudanças propostas antes que elas toquem no disco. Mantenha o agente útil sem abrir mão do controle.", icon: "✓" },
        { eyebrow: "CONTEXTO", title: "Controle de contexto", description: "Acompanhe uso de contexto, esforço do modelo, formato de tools e modo de raciocínio direto na sessão.", icon: "◌" },
        { eyebrow: "FLUXO", title: "Checkpoints", description: "Faça stage, unstage, diff e checkpoints a partir do mesmo loop de agente no terminal.", icon: "↻" },
      ],
    },
    code: {
      copy: "copiar",
      copied: "copiado",
    }
  }
};

const LanguageContext = createContext<{
  lang: Language;
  setLang: (l: Language) => void;
  t: typeof translations.en;
}>({
  lang: "en",
  setLang: () => { },
  t: translations.en,
});

const useLanguage = () => useContext(LanguageContext);

// --- Components ---

function IconBase({ className = "", children, fill = "none" }: SVGProps<SVGSVGElement> & { children: ReactNode }) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill={fill}
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

function Check(props: SVGProps<SVGSVGElement>) {
  return (
    <IconBase {...props}>
      <path d="m20 6-11 11-5-5" />
    </IconBase>
  );
}

function Cpu(props: SVGProps<SVGSVGElement>) {
  return (
    <IconBase {...props}>
      <rect x="6" y="6" width="12" height="12" rx="2" />
      <rect x="10" y="10" width="4" height="4" />
      <path d="M4 10h2M4 14h2M18 10h2M18 14h2M10 4v2M14 4v2M10 18v2M14 18v2" />
    </IconBase>
  );
}

function Github({ className = "" }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M12 .5a12 12 0 0 0-3.79 23.39c.6.11.82-.26.82-.58v-2.03c-3.34.73-4.04-1.42-4.04-1.42-.55-1.39-1.34-1.76-1.34-1.76-1.09-.75.08-.74.08-.74 1.2.09 1.84 1.24 1.84 1.24 1.07 1.83 2.81 1.3 3.49.99.11-.78.42-1.3.76-1.6-2.67-.31-5.47-1.34-5.47-5.93 0-1.31.47-2.38 1.24-3.22-.12-.31-.54-1.54.12-3.18 0 0 1.01-.32 3.3 1.23A11.44 11.44 0 0 1 12 5.98c1.02 0 2.04.14 3 .41 2.28-1.55 3.29-1.23 3.29-1.23.66 1.64.24 2.87.12 3.18.77.84 1.23 1.91 1.23 3.22 0 4.61-2.81 5.62-5.49 5.92.43.37.82 1.1.82 2.23v3.3c0 .32.21.7.83.58A12 12 0 0 0 12 .5Z" />
    </svg>
  );
}

function Layers(props: SVGProps<SVGSVGElement>) {
  return (
    <IconBase {...props}>
      <path d="m12 2 9 5-9 5-9-5 9-5Z" />
      <path d="m3 12 9 5 9-5" />
      <path d="m3 17 9 5 9-5" />
    </IconBase>
  );
}

function Lock(props: SVGProps<SVGSVGElement>) {
  return (
    <IconBase {...props}>
      <rect x="4" y="10" width="16" height="11" rx="2" />
      <path d="M8 10V7a4 4 0 0 1 8 0v3" />
      <path d="M12 14v3" />
    </IconBase>
  );
}

function Sparkles(props: SVGProps<SVGSVGElement>) {
  return (
    <IconBase {...props}>
      <path d="M12 3l1.5 4.5L18 9l-4.5 1.5L12 15l-1.5-4.5L6 9l4.5-1.5L12 3Z" />
      <path d="M19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8L19 15Z" />
      <path d="M5 14l.7 1.8L8 16.5l-2.3.7L5 19l-.7-1.8L2 16.5l2.3-.7L5 14Z" />
    </IconBase>
  );
}

function TerminalIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <IconBase {...props}>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="m7 9 3 3-3 3" />
      <path d="M12 15h5" />
    </IconBase>
  );
}

function AppleIcon({ className = "" }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M17.1 12.5c0-2.1 1.7-3.1 1.8-3.2-1-1.5-2.5-1.7-3.1-1.7-1.3-.1-2.5.8-3.2.8-.7 0-1.7-.8-2.8-.8-1.4 0-2.8.8-3.5 2.1-1.5 2.6-.4 6.4 1.1 8.5.7 1 1.6 2.2 2.7 2.1 1.1 0 1.5-.7 2.8-.7s1.7.7 2.8.7c1.2 0 1.9-1 2.6-2.1.8-1.2 1.2-2.4 1.2-2.5 0-.1-2.3-.9-2.4-3.2ZM15 6.2c.6-.7 1-1.7.9-2.7-.9 0-2 .6-2.6 1.3-.6.7-1.1 1.7-.9 2.7.9.1 1.9-.5 2.6-1.3Z" />
    </svg>
  );
}

function LinuxIcon({ className = "" }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d="M7.2 19.5c.8-1.6 1-3.8 1.2-6.1.2-2.4.5-5.9 3.6-5.9s3.4 3.5 3.6 5.9c.2 2.3.4 4.5 1.2 6.1"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path
        d="M8.7 17.3c-1.9.2-3.2 1.1-3.8 2.6 1.7.8 3.6.9 5.6.2M15.3 17.3c1.9.2 3.2 1.1 3.8 2.6-1.7.8-3.6.9-5.6.2"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path d="M10.6 10.7h.1M13.3 10.7h.1" stroke="currentColor" strokeWidth="2.3" strokeLinecap="round" />
      <path d="M10.4 13.8c.9.5 2.3.5 3.2 0" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      <path d="M10.1 6.2c.2-1.6.8-2.5 1.9-2.5s1.7.9 1.9 2.5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  );
}

function WindowsIcon({ className = "" }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M3 5.3 10.7 4v7.4H3V5.3Zm9-1.5L21 2.3v9.1h-9V3.8ZM3 12.7h7.7V20L3 18.7v-6Zm9 0h9v9l-9-1.5v-7.5Z" />
    </svg>
  );
}

function SourceIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <IconBase {...props}>
      <path d="m8 9-3 3 3 3" />
      <path d="m16 9 3 3-3 3" />
      <path d="m14 5-4 14" />
    </IconBase>
  );
}

function CintoSymbol({ className = "" }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 64 64" fill="none" aria-hidden="true">
      <defs>
        <linearGradient id="cinto-mark-gradient" x1="7" y1="55" x2="58" y2="7" gradientUnits="userSpaceOnUse">
          <stop stopColor="#082F3A" />
          <stop offset="0.48" stopColor="#5ED7D2" />
          <stop offset="1" stopColor="#B4F15B" />
        </linearGradient>
        <filter id="cinto-mark-glow" x="-20" y="-20" width="104" height="104" filterUnits="userSpaceOnUse">
          <feGaussianBlur stdDeviation="3" result="blur" />
          <feColorMatrix in="blur" type="matrix" values="0 0 0 0 0.37 0 0 0 0 0.84 0 0 0 0 0.82 0 0 0 0.52 0" />
          <feBlend in="SourceGraphic" />
        </filter>
      </defs>
      <g filter="url(#cinto-mark-glow)">
        <path
          d="M12 13.6 35.5 3.2 26.6 18.6 21.3 21.4v21.2l6.2 3.1 9.2 15.1L12 50.4 6.4 46.8V17.2L12 13.6Z"
          fill="url(#cinto-mark-gradient)"
        />
        <path
          d="M37.8 10.2 57.2 20.3 50.2 32.1 31 22.1 37.8 10.2Z"
          fill="url(#cinto-mark-gradient)"
          opacity="0.94"
        />
        <path
          d="M31.4 42.5 50.7 32.8 57.6 44.7 38.4 54.9 31.4 42.5Z"
          fill="url(#cinto-mark-gradient)"
          opacity="0.92"
        />
        <path d="M24.2 25.2 36.8 31.9 24.2 38.5V25.2Z" fill="#030506" opacity="0.98" />
      </g>
    </svg>
  );
}

function Background() {
  const { scrollYProgress } = useScroll();
  const y = useTransform(scrollYProgress, [0, 1], [0, 400]);

  return (
    <div className="pointer-events-none fixed inset-0 z-0 overflow-hidden bg-[#030506]">
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_-20%,rgba(94,215,210,0.15),transparent_70%)]" />
      <div className="absolute inset-0 opacity-50">
        <motion.div
          animate={{
            x: ["-15%", "15%"],
            rotate: [12, 18, 12],
            opacity: [0.3, 0.5, 0.3],
          }}
          transition={{ duration: 15, repeat: Infinity, ease: "easeInOut" }}
          className="absolute -left-[20%] top-[-10%] h-[120vh] w-[140vw] bg-[radial-gradient(ellipse_at_center,rgba(94,215,210,0.18),transparent_50%)] [mask-image:repeating-linear-gradient(110deg,black_0px,black_1px,transparent_2px,transparent_30px)]"
        />
        <motion.div
          animate={{
            x: ["8%", "-8%"],
            rotate: [-8, -12, -8],
            opacity: [0.2, 0.4, 0.2],
          }}
          transition={{ duration: 18, repeat: Infinity, ease: "easeInOut" }}
          className="absolute -right-[20%] top-[10%] h-[130vh] w-[150vw] bg-[radial-gradient(ellipse_at_center,rgba(180,241,91,0.12),transparent_60%)] [mask-image:repeating-linear-gradient(-70deg,black_0px,black_2px,transparent_4px,transparent_50px)]"
        />
      </div>
      <AnimatedGridPattern
        className="absolute inset-0 opacity-[0.38]"
        size={48}
        duration={16}
        stroke="rgba(255, 255, 255, 0.18)"
        style={{
          maskImage: "radial-gradient(ellipse at 50% 40%, black 0%, transparent 75%)",
          WebkitMaskImage:
            "radial-gradient(ellipse at 50% 40%, black 0%, transparent 75%)",
          mixBlendMode: "screen",
          filter: "drop-shadow(0 0 18px rgba(126, 231, 226, 0.18))",
        }}
      />
      <motion.div
        style={{ y }}
        className="absolute left-1/2 top-[15%] h-[40rem] w-[40rem] -translate-x-1/2 rounded-full bg-[#5ED7D2]/5 blur-[120px]"
      />
      <div
        className="absolute inset-0 opacity-[0.02] mix-blend-overlay"
        style={{
          backgroundImage: `url("data:image/svg+xml,%3Csvg viewBox='0 0 250 250' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noiseFilter'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.65' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noiseFilter)'/%3E%3C/svg%3E")`
        }}
      />
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,transparent_0%,#030506_90%)]" />
    </div>
  );
}

function CintoMark() {
  const { t } = useLanguage();
  return (
    <div className="flex items-center gap-3">
      <motion.div whileHover={{ scale: 1.06, rotate: -2 }} className="relative">
        <div className="absolute inset-0 rounded-xl bg-[#5ED7D2]/18 blur-xl" />
        <CintoSymbol className="relative h-9 w-9" />
      </motion.div>
      <div className="leading-none">
        <span className="block text-sm font-semibold tracking-wide text-white">Cinto</span>
        <span className="mt-1 block text-[9px] uppercase tracking-[0.28em] text-[#5ED7D2]/55">
          {t.hero.tag.split("·")[1].trim()}
        </span>
      </div>
    </div>
  );
}

function Header() {
  const { lang, setLang, t } = useLanguage();
  return (
    <header className="fixed inset-x-0 top-0 z-50 border-b border-white/[0.06] bg-black/20 backdrop-blur-xl">
      <div className="mx-auto flex max-w-7xl items-center justify-between px-6 py-4">
        <CintoMark />
        <nav className="hidden items-center gap-8 text-sm text-white/48 md:flex">
          <a className="transition hover:text-white" href="#features">{t.nav.features}</a>
          <a className="transition hover:text-[#7FE7E2]" href="#stack">{t.nav.stack}</a>
          <a className="transition hover:text-white" href="#workflow">{t.nav.workflow}</a>
          <a className="transition hover:text-white" href="#crp">{t.nav.crp}</a>
          <a className="transition hover:text-white" href="#install">{t.nav.install}</a>
        </nav>
        <div className="flex items-center gap-4">
          <button
            onClick={() => setLang(lang === "en" ? "pt" : "en")}
            className="flex h-8 w-14 items-center rounded-full border border-white/10 bg-white/5 p-1 transition-colors hover:border-[#5ED7D2]/40"
          >
            <motion.div
              animate={{ x: lang === "en" ? 0 : 24 }}
              className="flex h-6 w-6 items-center justify-center rounded-full bg-white text-[10px] font-bold text-black"
            >
              {lang.toUpperCase()}
            </motion.div>
          </button>
          <motion.a
            whileHover={{ scale: 1.04 }}
            whileTap={{ scale: 0.98 }}
            href="https://github.com/Jshebb/cinto"
            className="inline-flex items-center gap-2 rounded-full bg-white px-4 py-2 text-sm font-semibold text-black"
          >
            <Github className="h-4 w-4" />
            <span className="hidden sm:inline">{t.nav.github}</span>
          </motion.a>
        </div>
      </div>
    </header>
  );
}

function TypewriterHighlight() {
  const { t } = useLanguage();
  const phrases = t.hero.typewriter;

  const [phraseIndex, setPhraseIndex] = useState(0);
  const [text, setText] = useState("");
  const [isDeleting, setIsDeleting] = useState(false);

  useEffect(() => {
    const currentPhrase = phrases[phraseIndex];

    const typingSpeed = isDeleting ? 32 : 58;
    const pauseTime = 1200;

    const timeout = window.setTimeout(
      () => {
        if (!isDeleting && text === currentPhrase) {
          setIsDeleting(true);
          return;
        }

        if (isDeleting && text === "") {
          setIsDeleting(false);
          setPhraseIndex((current) => (current + 1) % phrases.length);
          return;
        }

        setText((current) =>
          isDeleting
            ? currentPhrase.slice(0, current.length - 1)
            : currentPhrase.slice(0, current.length + 1)
        );
      },
      !isDeleting && text === currentPhrase ? pauseTime : typingSpeed
    );

    return () => window.clearTimeout(timeout);
  }, [text, isDeleting, phraseIndex]);

  return (
    <span className="block min-h-[0.95em] xl:whitespace-nowrap">
      <span className="bg-gradient-to-r from-[#7FE7E2] via-[#9BEF9A] to-[#B4F15B] bg-clip-text text-transparent">
        {text || "\u00A0"}
      </span>

      <motion.span
        className="ml-3 inline-block h-[0.82em] w-[4px] translate-y-[0.09em] bg-gradient-to-b from-[#7FE7E2] to-[#B4F15B]"
        animate={{ opacity: [0, 1, 1, 0] }}
        transition={{ duration: 1.1, repeat: Infinity }}
      />
    </span>
  );
}

function Hero() {
  const { t } = useLanguage();

  return (
    <section className="relative z-10 flex min-h-screen flex-col items-center justify-center px-6 pb-28 pt-32 text-center sm:pt-40">
      <motion.div
        initial={{ opacity: 0, y: 14 }}
        animate={{ opacity: 1, y: 0 }}
        className="mx-auto mb-7 inline-flex items-center gap-2 rounded-full border border-[#5ED7D2]/14 bg-[#5ED7D2]/[0.045] px-4 py-2 font-mono text-xs uppercase tracking-[0.18em] text-white/52"
      >
        <span className="h-1.5 w-1.5 rounded-full bg-[#B4F15B]/70" />
        {t.hero.tag}
      </motion.div>

      <h1 className="mx-auto max-w-[1180px] text-center text-[clamp(3.2rem,6.8vw,7.2rem)] font-semibold leading-[0.92] tracking-[-0.075em] text-white">
        <motion.span
          className="block text-white"
          initial={{ opacity: 0, y: 28, filter: "blur(12px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          transition={{ duration: 0.9 }}
        >
          {t.hero.headline[0]}
        </motion.span>

        <motion.span
          className="block"
          initial={{ opacity: 0, y: 28, filter: "blur(12px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          transition={{ duration: 0.9, delay: 0.15 }}
        >
          <TypewriterHighlight />
        </motion.span>
      </h1>

      <motion.p
        initial={{ opacity: 0, y: 18 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.34 }}
        className="mx-auto mt-8 max-w-4xl text-balance text-lg leading-8 text-white/52 md:text-xl"
      >
        {t.hero.description}
      </motion.p>

      <motion.div
        initial={{ opacity: 0, y: 18 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.48 }}
        className="mt-10 flex flex-col items-center justify-center gap-4 sm:flex-row"
      >
        <motion.a
          whileHover={{ scale: 1.04 }}
          whileTap={{ scale: 0.98 }}
          href="#install"
          className="inline-flex items-center gap-2 rounded-full bg-gradient-to-r from-[#5ED7D2] to-[#B4F15B] px-6 py-3 text-sm font-semibold text-[#061011]"
        >
          <TerminalIcon className="h-4 w-4" />
          {t.hero.cta_install}
        </motion.a>

        <motion.a
          whileHover={{ scale: 1.04 }}
          whileTap={{ scale: 0.98 }}
          href="https://github.com/Jshebb/cinto"
          className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/[0.04] px-6 py-3 text-sm font-semibold text-white/78 backdrop-blur"
        >
          <Github className="h-4 w-4" />
          {t.hero.cta_github}
        </motion.a>
      </motion.div>
    </section>
  );
}



function TerminalShowcase() {
  const { t } = useLanguage();

  return (
    <div className="mx-auto mt-20 max-w-[1500px] px-6">
      <div className="grid items-center gap-12 md:grid-cols-[0.82fr_1.18fr] lg:gap-16">
        <div className="text-left">
          <div className="mb-6 text-xs font-semibold uppercase tracking-[0.28em] text-[#5ED7D2]/55">
            {t.walkthrough.setup_tag}
          </div>

          <h2 className="text-4xl font-bold leading-tight text-white md:text-5xl lg:text-6xl">
            {t.walkthrough.setup_title}
          </h2>

          <p className="mt-6 max-w-xl text-lg text-white/60">
            {t.walkthrough.setup_description}
          </p>

          <ul className="mt-8 space-y-4">
            {t.walkthrough.setup_items.map(([title, text]) => (
              <li key={title} className="flex items-start gap-4">
                <span className="mt-1 text-[#7FE7E2]">▣</span>
                <div>
                  <div className="font-semibold">{title}</div>
                  <div className="text-sm text-white/60">{text}</div>
                </div>
              </li>
            ))}
          </ul>
        </div>

        <div className="relative flex items-center justify-center md:-mr-10 lg:-mr-16">
          <div className="absolute -inset-8 rounded-[2rem] bg-[#5ED7D2]/8 blur-3xl" />
          <div className="relative w-full overflow-hidden rounded-3xl border border-white/10 bg-[#000] shadow-2xl shadow-black/60">
            <img
              src={terminalSetup}
              alt={t.walkthrough.setup_alt}
              className="h-full w-full rounded-3xl object-cover"
            />
            <div className="pointer-events-none absolute inset-0 rounded-3xl ring-1 ring-white/8" />
          </div>
        </div>
      </div>
    </div>
  );
}

function TerminalUse() {
  const { t } = useLanguage();

  return (
    <div className="mx-auto mt-28 max-w-[1500px] px-6">
      <div className="grid items-center gap-12 md:grid-cols-[1.18fr_0.82fr] lg:gap-16">
        <div className="relative flex items-center justify-center md:-ml-10 lg:-ml-16">
          <div className="absolute -inset-8 rounded-[2rem] bg-[#5ED7D2]/8 blur-3xl" />
          <div className="relative w-full overflow-hidden rounded-3xl border border-white/10 bg-[#000] shadow-2xl shadow-black/60">
            <img
              src={terminalUse}
              alt={t.walkthrough.use_alt}
              className="h-full w-full rounded-3xl object-cover"
            />
            <div className="pointer-events-none absolute inset-0 rounded-3xl ring-1 ring-white/8" />
          </div>
        </div>

        <div className="text-left md:pl-4 lg:pl-8">
          <div className="mb-6 text-xs font-semibold uppercase tracking-[0.28em] text-[#5ED7D2]/55">
            {t.walkthrough.use_tag}
          </div>

          <h2 className="text-4xl font-bold leading-tight text-white md:text-5xl lg:text-6xl">
            {t.walkthrough.use_title}
          </h2>

          <p className="mt-6 max-w-xl text-lg text-white/60">
            {t.walkthrough.use_description}
          </p>

          <ul className="mt-8 space-y-4">
            {t.walkthrough.use_items.map(([title, text]) => (
              <li key={title} className="flex items-start gap-4">
                <span className="mt-1 text-[#7FE7E2]">▣</span>
                <div>
                  <div className="font-semibold">{title}</div>
                  <div className="text-sm text-white/60">{text}</div>
                </div>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
}

function CrpSection() {
  const { t } = useLanguage();

  return (
    <section
      id="crp"
      className="relative z-10 scroll-mt-24 px-6 py-32 md:py-44"
    >
      <div className="mx-auto max-w-7xl">
        <div className="mx-auto max-w-3xl text-center">
          <div className="mb-5 text-xs font-semibold uppercase tracking-[0.28em] text-[#5ED7D2]/55">
            {t.crp.tag}
          </div>

          <h2 className="text-4xl font-semibold tracking-[-0.055em] text-white sm:text-5xl lg:text-6xl">
            {t.crp.title}
          </h2>

          <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-white/48">
            {t.crp.description}
          </p>
        </div>

        <div className="mx-auto mt-16 w-full max-w-5xl">
          <TerminalCodeBlock title={t.crp.code_title} code={t.crp.example} />
        </div>
      </div>
    </section>
  );
}

function Features() {
  const { t } = useLanguage();
  const icons = [TerminalIcon, Cpu, Lock, Layers];
  return (
    <section id="features" className="relative z-10 scroll-mt-24 px-6 py-28">
      <div className="mx-auto max-w-7xl">
        <div className="mx-auto max-w-3xl text-center">
          <div className="mb-5 text-xs font-semibold uppercase tracking-[0.28em] text-[#5ED7D2]/55">{t.features.tag}</div>
          <h2 className="text-4xl font-semibold tracking-[-0.055em] text-white sm:text-6xl">{t.features.title}</h2>
          <p className="mt-6 text-lg leading-8 text-white/45">{t.features.description}</p>
        </div>
        <div className="mt-16 grid gap-4 md:grid-cols-2 lg:grid-cols-4">
          {t.features.items.map((feature, index) => {
            const Icon = icons[index];
            return (
              <motion.div
                key={feature.title}
                initial={{ opacity: 0, y: 28 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true, margin: "-120px" }}
                transition={{ delay: index * 0.06 }}
                className="group rounded-3xl border border-white/10 bg-white/[0.028] p-6 backdrop-blur transition hover:border-[#5ED7D2]/20 hover:bg-white/[0.05]"
              >
                <div className="mb-8 flex h-12 w-12 items-center justify-center rounded-2xl border border-white/10 bg-white/[0.045] text-white/70 transition group-hover:text-[#7FE7E2]">
                  <Icon className="h-6 w-6" />
                </div>
                <h3 className="text-xl font-semibold text-white/88">{feature.title}</h3>
                <p className="mt-4 leading-7 text-white/42">{feature.text}</p>
              </motion.div>
            );
          })}
        </div>
      </div>
    </section>
  );
}

function Workflow() {
  const { t } = useLanguage();
  return (
    <section id="workflow" className="relative z-10 scroll-mt-24 px-6 pb-28">
      <div className="mx-auto grid max-w-7xl gap-12 lg:grid-cols-[0.9fr_1.1fr] lg:items-center">
        <div>
          <div className="mb-5 text-xs font-semibold uppercase tracking-[0.28em] text-[#5ED7D2]/55">{t.workflow.tag}</div>
          <h2 className="text-4xl font-semibold tracking-[-0.055em] text-white sm:text-6xl">{t.workflow.title}</h2>
          <p className="mt-6 text-lg leading-8 text-white/45">{t.workflow.description}</p>
        </div>
        <div className="rounded-3xl border border-white/10 bg-black/35 p-4 backdrop-blur">
          <div className="space-y-3">
            {t.workflow.steps.map(([number, title, text], index) => (
              <motion.div
                key={title}
                initial={{ opacity: 0, x: 24 }}
                whileInView={{ opacity: 1, x: 0 }}
                viewport={{ once: true }}
                transition={{ delay: index * 0.08 }}
                className="rounded-2xl border border-white/10 bg-white/[0.03] p-5 transition hover:border-[#5ED7D2]/18"
              >
                <div className="flex gap-4">
                  <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-white/10 bg-white/[0.04] font-mono text-sm text-[#E6D36D]/80">
                    {number}
                  </div>
                  <div>
                    <div className="font-mono text-sm uppercase tracking-[0.16em] text-white/70">{title}</div>
                    <div className="mt-2 leading-7 text-white/42">{text}</div>
                  </div>
                </div>
              </motion.div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}

function InstallCommandCard() {
  const { t } = useLanguage();

  return (
    <motion.div
      initial={{ opacity: 0, y: 24, filter: "blur(10px)" }}
      whileInView={{ opacity: 1, y: 0, filter: "blur(0px)" }}
      viewport={{ once: true, margin: "-100px" }}
      transition={{ duration: 0.7 }}
      className="mx-auto mt-12 max-w-4xl overflow-hidden rounded-lg border border-white/[0.06] bg-[#111315]/90 shadow-2xl shadow-black/50 backdrop-blur"
    >
      <div className="flex items-center justify-between border-b border-white/[0.06] px-5 py-4">
        <div className="flex items-center gap-2">
          <span className="h-2.5 w-2.5 rounded-full bg-[#E6D36D]/45" />
          <span className="h-2.5 w-2.5 rounded-full bg-[#5ED7D2]/45" />
        </div>

        <div className="font-mono text-xs text-white/30">{t.install.terminal_title}</div>
      </div>

      <div className="space-y-4 p-6 font-mono text-sm leading-7 text-white/68 md:p-8">
        {t.install.terminal_commands.map((command) => (
          <div key={command} className="break-all">
            <span className="text-[#E6D36D]">$</span>{" "}
            {command}
          </div>
        ))}
      </div>
    </motion.div>
  );
}

function InstallMethodIcon({ index, title }: { index: number; title: string }) {
  const iconClass = "h-5 w-5";

  if (index === 0) {
    return (
      <div className="flex h-10 w-10 items-center justify-center rounded-lg border border-white/[0.06] bg-white/[0.035] text-[#7FE7E2]">
        <SourceIcon className={iconClass} />
      </div>
    );
  }

  if (index === 1) {
    return (
      <div className="flex gap-2" aria-label={title}>
        <div className="flex h-10 w-10 items-center justify-center rounded-lg border border-white/[0.06] bg-white/[0.035] text-white/78">
          <AppleIcon className={iconClass} />
        </div>
        <div className="flex h-10 w-10 items-center justify-center rounded-lg border border-white/[0.06] bg-white/[0.035] text-[#7FE7E2]">
          <LinuxIcon className={iconClass} />
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-10 w-10 items-center justify-center rounded-lg border border-white/[0.06] bg-white/[0.035] text-[#7FE7E2]">
      <WindowsIcon className={iconClass} />
    </div>
  );
}

function InstallMethodCards() {
  const { t } = useLanguage();
  const methods = t.install.methods;

  return (
    <div className="mt-14 grid gap-4 md:grid-cols-3">
      {methods.map((method, index) => (
        <motion.div
          key={method.title}
          initial={{ opacity: 0, y: 18 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true, margin: "-80px" }}
          transition={{ delay: index * 0.06 }}
          className="group min-w-0 rounded-lg border border-white/[0.06] bg-white/[0.025] p-5 transition hover:border-[#5ED7D2]/16 hover:bg-white/[0.045]"
        >
          <div className="mb-5">
            <InstallMethodIcon index={index} title={method.title} />
          </div>

          <h3 className="font-semibold text-white/88">{method.title}</h3>

          <p className="mt-3 text-sm leading-6 text-white/44">
            {method.text}
          </p>

          <div className="mt-5 min-h-[4.5rem] min-w-0 break-all rounded-md border border-white/[0.06] bg-black/25 px-3 py-3 font-mono text-xs leading-6 text-[#7FE7E2]/70">
            {method.command}
          </div>
        </motion.div>
      ))}
    </div>
  );
}

function InstallDetails() {
  const { t } = useLanguage();
  const quickstart = t.install.quickstart;
  const commands = t.install.commands;

  return (
    <div className="mt-14 grid gap-4 lg:grid-cols-[0.85fr_1.15fr]">
      <div className="rounded-3xl border border-white/[0.06] bg-white/[0.02] p-7">
        <h3 className="text-xl font-semibold text-white/88">{t.install.quickstart_title}</h3>

        <ol className="mt-6 space-y-5">
          {quickstart.map((item, index) => (
            <li key={item} className="flex gap-4 text-sm leading-6 text-white/48">
              <span className="font-mono text-[#E6D36D]/80">
                {index + 1}.
              </span>
              <span>{item}</span>
            </li>
          ))}
        </ol>
      </div>

      <div className="rounded-3xl border border-white/[0.06] bg-white/[0.02] p-7">
        <h3 className="text-xl font-semibold text-white/88">{t.install.after_title}</h3>

        <div className="mt-6 divide-y divide-white/[0.06]">
          {commands.map(([command, text]) => (
            <div
              key={command}
              className="flex flex-col gap-1 py-4 sm:flex-row sm:items-center sm:justify-between"
            >
              <code className="break-all font-mono text-sm text-[#7FE7E2]/82">
                {command}
              </code>
              <span className="text-sm text-white/38">
                {text}
              </span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function InstallSafetyNote() {
  const { t } = useLanguage();

  return (
    <div className="mx-auto mt-8 max-w-4xl rounded-lg border border-[#E6D36D]/10 bg-[#E6D36D]/[0.025] px-5 py-4 text-center text-sm leading-6 text-white/42">
      {t.install.safety}
    </div>
  );
}

function Install() {
  const { t } = useLanguage();

  return (
    <section id="install" className="relative z-10 scroll-mt-24 px-6 py-32">
      <div className="mx-auto max-w-7xl">
        <div className="mx-auto max-w-3xl text-center">
          <div className="mb-5 inline-flex items-center gap-2 rounded-lg border border-white/[0.06] bg-black/25 px-4 py-2 text-sm text-white/52">
            <Sparkles className="h-4 w-4 text-[#7FE7E2]" />
            {t.install.tag}
          </div>

          <h2 className="text-4xl font-semibold tracking-[-0.055em] text-white sm:text-6xl">
            {t.install.title}
          </h2>

          <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-white/48">
            {t.install.description}
          </p>

          <div className="mt-8 flex flex-wrap justify-center gap-3">
            {t.install.badges.map((item) => (
              <span
                key={item}
                className="inline-flex items-center gap-2 rounded-lg border border-white/[0.06] bg-black/20 px-4 py-2 text-sm text-white/58"
              >
                <Check className="h-4 w-4 text-[#7FE7E2]" />
                {item}
              </span>
            ))}
          </div>
        </div>

        <InstallCommandCard />
        <InstallMethodCards />
        <InstallDetails />
        <InstallSafetyNote />
      </div>
    </section>
  );
}

function StackMarquee() {
  const { t } = useLanguage();
  const cards = t.stack.cards;

  return (
    <section
      id="stack"
      className="relative z-10 scroll-mt-24 border-y border-white/[0.07] bg-white/[0.018] py-28"
    >
      <div className="mx-auto max-w-7xl px-6">
        <div className="mx-auto max-w-3xl text-center">
          <div className="mb-5 text-xs font-semibold uppercase tracking-[0.28em] text-[#5ED7D2]/55">
            {t.nav.stack}
          </div>

          <h2 className="text-4xl font-semibold tracking-[-0.055em] text-white sm:text-5xl lg:text-6xl">
            {t.stack.title}
          </h2>

          <p className="mx-auto mt-5 max-w-2xl text-lg leading-8 text-white/45">
            {t.stack.description}
          </p>
        </div>

        <div className="mt-16 grid border border-white/[0.07] md:grid-cols-2 lg:grid-cols-3">
          {cards.map((card, index) => (
            <motion.div
              key={card.title}
              initial={{ opacity: 0, y: 24, filter: "blur(10px)" }}
              whileInView={{ opacity: 1, y: 0, filter: "blur(0px)" }}
              viewport={{ once: true, margin: "-100px" }}
              transition={{ delay: index * 0.045 }}
              className="group relative min-h-[230px] border-b border-r border-white/[0.07] bg-black/10 p-8 transition hover:bg-white/[0.035] lg:[&:nth-child(3n)]:border-r-0 lg:[&:nth-last-child(-n+3)]:border-b-0 md:[&:nth-child(2n)]:border-r-0 lg:[&:nth-child(2n)]:border-r lg:[&:nth-child(3n)]:border-r-0"
            >
              <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_80%_0%,rgba(94,215,210,0.10),transparent_34%)] opacity-0 transition duration-500 group-hover:opacity-100" />

              <div className="relative">
                <div className="mb-8 flex h-10 w-10 items-center justify-center rounded-xl border border-white/10 bg-white/[0.035] font-mono text-sm text-[#7FE7E2]">
                  {card.icon}
                </div>

                <div className="mb-3 font-mono text-xs uppercase tracking-[0.22em] text-[#5ED7D2]/45">
                  {card.eyebrow}
                </div>

                <h3 className="text-xl font-semibold text-white/90">
                  {card.title}
                </h3>

                <p className="mt-4 max-w-sm leading-7 text-white/44">
                  {card.description}
                </p>
              </div>
            </motion.div>
          ))}
        </div>

        <p className="mx-auto mt-8 max-w-3xl text-center text-xs leading-6 text-white/25">
          {t.stack.disclaimer}
        </p>
      </div>
    </section>
  );
}

function Footer() {
  const { t } = useLanguage();
  return (
    <footer className="relative z-10 border-t border-white/[0.07] px-6 py-10">
      <div className="mx-auto flex max-w-7xl flex-col gap-6 text-white/35 md:flex-row md:items-center md:justify-between">
        <CintoMark />
        <div className="flex flex-wrap gap-6 text-sm">
          <a className="hover:text-white" href="#features">{t.nav.features}</a>
          <a className="hover:text-white" href="#stack">{t.nav.stack}</a>
          <a className="hover:text-white" href="#install">{t.nav.install}</a>
        </div>
      </div>
    </footer>
  );
}

function Reveal({
  children,
  delay = 0,
}: {
  children: React.ReactNode;
  delay?: number;
}) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 80, filter: "blur(14px)" }}
      whileInView={{ opacity: 1, y: 0, filter: "blur(0px)" }}
      viewport={{ once: true, margin: "-120px" }}
      transition={{
        duration: 0.9,
        delay,
        ease: "easeOut",
      }}
    >
      {children}
    </motion.div>
  );
}

function ProductWalkthrough() {
  return (
    <section className="relative z-10 space-y-40 px-6 py-28 md:space-y-52 md:py-40">
      <Reveal>
        <TerminalShowcase />
      </Reveal>

      <Reveal delay={0.08}>
        <TerminalUse />
      </Reveal>
    </section>
  );
}

function TerminalCodeBlock({
  title = "crp.turn",
  code,
}: {
  title?: string;
  code: string;
}) {
  const { t } = useLanguage();
  const [copied, setCopied] = useState(false);

  async function copyCode() {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  }

  return (
    <div className="relative overflow-hidden rounded-[2rem] border border-white/10 bg-[#111315]/95 shadow-2xl shadow-black/60">
      <div className="absolute -inset-20 bg-[radial-gradient(circle_at_72%_12%,rgba(94,215,210,0.16),transparent_34%),radial-gradient(circle_at_15%_86%,rgba(180,241,91,0.10),transparent_34%)]" />

      <div className="relative flex items-center justify-between border-b border-white/10 px-5 py-4">
        <div className="flex items-center gap-2">
          <CintoSymbol className="h-4 w-4 text-[#7FE7E2]" />
        </div>

        <div className="flex items-center gap-3">
          <div className="font-mono text-xs text-white/30">{title}</div>

          <button
            onClick={copyCode}
            className="rounded-full border border-white/10 bg-white/[0.04] px-3 py-1 text-xs text-white/45 transition hover:border-[#5ED7D2]/30 hover:text-[#7FE7E2]"
          >
            {copied ? t.code.copied : t.code.copy}
          </button>
        </div>
      </div>

      <pre className="relative max-h-[640px] overflow-auto whitespace-pre-wrap break-words p-6 text-left font-mono text-sm leading-7 text-white/62 md:p-8">
        <code>
          {code.split("\n").map((line, index) => {
            const isHeading = [
              "TASK_INTERPRETATION",
              "RELEVANT_FILES",
              "PROPOSED_APPROACH",
              "TOOL_EXECUTION",
              "FINAL_RESPONSE",
            ].includes(line);

            const isTool =
              line.includes("read_file") ||
              line.includes("write_file") ||
              line.includes("search");

            const isList = line.startsWith("-") || /^[0-9]\./.test(line);

            return (
              <span
                key={`${line}-${index}`}
                className={
                  isHeading
                    ? "block pt-3 text-[#7FE7E2]"
                    : isTool
                      ? "block text-[#E6D36D]/85"
                      : isList
                        ? "block text-white/52"
                        : "block"
                }
              >
                {line || " "}
              </span>
            );
          })}
        </code>
      </pre>
    </div>
  );
}

export default function App() {
  const [lang, setLang] = useState<Language>("en");
  const t = translations[lang];

  return (
    <LanguageContext.Provider value={{ lang, setLang, t }}>
      <main className="min-h-screen overflow-x-hidden bg-[#030506] text-white selection:bg-[#7FE7E2] selection:text-black">
        <Background />
        <Header />
        <Hero />
        <ProductWalkthrough />
        <CrpSection />
        <StackMarquee />
        <Features />
        <Workflow />
        <Install />
        <Footer />
      </main>
    </LanguageContext.Provider>
  );
}
