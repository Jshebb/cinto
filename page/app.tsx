import { motion, useScroll, useTransform } from "framer-motion";

function IconBase({ className = "", children, fill = "none" }) {
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

function ArrowRight(props) {
  return (
    <IconBase {...props}>
      <path d="M5 12h14" />
      <path d="m12 5 7 7-7 7" />
    </IconBase>
  );
}

function Check(props) {
  return (
    <IconBase {...props}>
      <path d="m20 6-11 11-5-5" />
    </IconBase>
  );
}

function Cpu(props) {
  return (
    <IconBase {...props}>
      <rect x="6" y="6" width="12" height="12" rx="2" />
      <rect x="10" y="10" width="4" height="4" />
      <path d="M4 10h2M4 14h2M18 10h2M18 14h2M10 4v2M14 4v2M10 18v2M14 18v2" />
    </IconBase>
  );
}

function FileSearch(props) {
  return (
    <IconBase {...props}>
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <path d="M14 2v6h6" />
      <circle cx="11" cy="14" r="3" />
      <path d="m13.5 16.5 2 2" />
    </IconBase>
  );
}

function Github({ className = "" }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M12 .5a12 12 0 0 0-3.79 23.39c.6.11.82-.26.82-.58v-2.03c-3.34.73-4.04-1.42-4.04-1.42-.55-1.39-1.34-1.76-1.34-1.76-1.09-.75.08-.74.08-.74 1.2.09 1.84 1.24 1.84 1.24 1.07 1.83 2.81 1.3 3.49.99.11-.78.42-1.3.76-1.6-2.67-.31-5.47-1.34-5.47-5.93 0-1.31.47-2.38 1.24-3.22-.12-.31-.54-1.54.12-3.18 0 0 1.01-.32 3.3 1.23A11.44 11.44 0 0 1 12 5.98c1.02 0 2.04.14 3 .41 2.28-1.55 3.29-1.23 3.29-1.23.66 1.64.24 2.87.12 3.18.77.84 1.23 1.91 1.23 3.22 0 4.61-2.81 5.62-5.49 5.92.43.37.82 1.1.82 2.23v3.3c0 .32.21.7.83.58A12 12 0 0 0 12 .5Z" />
    </svg>
  );
}

function Layers3(props) {
  return (
    <IconBase {...props}>
      <path d="m12 2 9 5-9 5-9-5 9-5Z" />
      <path d="m3 12 9 5 9-5" />
      <path d="m3 17 9 5 9-5" />
    </IconBase>
  );
}

function LockKeyhole(props) {
  return (
    <IconBase {...props}>
      <rect x="4" y="10" width="16" height="11" rx="2" />
      <path d="M8 10V7a4 4 0 0 1 8 0v3" />
      <circle cx="12" cy="15" r="1" />
      <path d="M12 16v2" />
    </IconBase>
  );
}

function Play(props) {
  return (
    <IconBase {...props} fill="currentColor">
      <path d="M8 5v14l11-7Z" />
    </IconBase>
  );
}

function ShieldCheck(props) {
  return (
    <IconBase {...props}>
      <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10Z" />
      <path d="m9 12 2 2 4-4" />
    </IconBase>
  );
}

function Sparkles(props) {
  return (
    <IconBase {...props}>
      <path d="M12 3l1.5 4.5L18 9l-4.5 1.5L12 15l-1.5-4.5L6 9l4.5-1.5L12 3Z" />
      <path d="M19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8L19 15Z" />
      <path d="M5 14l.7 1.8L8 16.5l-2.3.7L5 19l-.7-1.8L2 16.5l2.3-.7L5 14Z" />
    </IconBase>
  );
}

function TerminalSquare(props) {
  return (
    <IconBase {...props}>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="m7 9 3 3-3 3" />
      <path d="M12 15h5" />
    </IconBase>
  );
}

function WorkflowIcon(props) {
  return (
    <IconBase {...props}>
      <path d="M6 3v6" />
      <path d="M18 15v6" />
      <circle cx="6" cy="12" r="3" />
      <circle cx="18" cy="12" r="3" />
      <path d="M9 12h6" />
    </IconBase>
  );
}

function Zap(props) {
  return (
    <IconBase {...props}>
      <path d="M13 2 3 14h8l-1 8 11-13h-8l1-7Z" />
    </IconBase>
  );
}

const fadeUp = {
  hidden: { opacity: 0, y: 26, filter: "blur(10px)" },
  visible: { opacity: 1, y: 0, filter: "blur(0px)" },
};

const stagger = {
  hidden: {},
  visible: { transition: { staggerChildren: 0.08 } },
};

const integrations = [
  "OpenAI Tools",
  "Harmony",
  "OpenAI-compatible APIs",
  "LM Studio",
  "Ollama",
  "Hugging Face",
  "Qwen",
  "Gemma",
  "GPT-OSS",
  "DeepSeek",
  "CRP",
  "Local Models",
  "Approval Diffs",
  "Rust TUI",
];

const featureCards = [
  {
    icon: TerminalSquare,
    title: "Terminal-native workspace",
    description:
      "A focused TUI for coding agents: prompts, todos, tools, diffs and session state in one visible loop.",
  },
  {
    icon: Cpu,
    title: "Small-model first",
    description:
      "Built around local endpoints and compact models so useful agents can run on real developer machines.",
  },
  {
    icon: ShieldCheck,
    title: "Approval-first edits",
    description:
      "Agents can reason and propose, but edits stay inspectable before they touch your repository.",
  },
  {
    icon: Layers3,
    title: "Structured reasoning",
   {
  "task_progress": [
    {
      "id": 1,
      "description": "Examine the current landing page in pages directory",
      "status": "complete"
    },
    {
      "id": 2,
      "description": "Identify areas for refinement and improvement",
      "status": "complete"
    },
    {
      "id": 3,
      "description": "Implement refined landing page based on analysis",
      "status": "in_progress"
    },
    {
      "id": 4,
      "description": "Verify the landing page meets requirements",
      "status": "pending"
    }
  ]
}