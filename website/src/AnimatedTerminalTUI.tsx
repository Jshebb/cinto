import { useEffect, useState } from "react";
import { motion } from "framer-motion";

const COMMAND = "Refactor the Hero component inside main.tsx";
const OUTPUT_LINES = [
  "Inspecting Hero component structure...",
  "Identifying reusable sections and simplifying layout...",
  "Refactor ready: App.tsx and HeroTerminal now decoupled.",
];

function useTyping(text: string, speed = 35) {
  const [displayed, setDisplayed] = useState("");

  useEffect(() => {
    let i = 0;
    setDisplayed("");
    const interval = setInterval(() => {
      setDisplayed((prev) => prev + text[i]);
      i += 1;
      if (i >= text.length) clearInterval(interval);
    }, speed);
    return () => clearInterval(interval);
  }, [text, speed]);

  return displayed;
}

function Cursor() {
  return (
    <motion.span
      className="ml-1 inline-block h-4 w-2 rounded-[2px] bg-[#7FE7E2] align-middle"
      animate={{ opacity: [0, 1, 1, 0] }}
      transition={{ duration: 1, repeat: Infinity }}
    />
  );
}

export default function AnimatedTerminalTUI() {
  const typed = useTyping(COMMAND, 35);
  const typingDone = typed.length === COMMAND.length;
  const [visibleLines, setVisibleLines] = useState(0);

  useEffect(() => {
    if (!typingDone) return;
    let index = 0;
    const timeout = setTimeout(() => {
      setVisibleLines(1);
      index = 1;
      const interval = setInterval(() => {
        index += 1;
        setVisibleLines(index);
        if (index >= OUTPUT_LINES.length) {
          clearInterval(interval);
        }
      }, 500);
    }, 300);

    return () => clearTimeout(timeout);
  }, [typingDone]);

  return (
    <div className="relative mx-auto w-full max-w-3xl overflow-hidden rounded-2xl border border-white/10 bg-[#0b0f14] shadow-2xl">
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(94,215,210,0.12),transparent_45%)]" />
      <div className="absolute inset-0 opacity-[0.08] bg-[linear-gradient(transparent_1px,rgba(255,255,255,0.04)_1px)] bg-[size:100%_22px]" />

      <div className="relative flex items-center gap-3 border-b border-white/10 bg-[#11161d]/80 px-4 py-3">
        <div className="flex items-center gap-2">
          <span className="h-3 w-3 rounded-full bg-[#ff5f57]" />
          <span className="h-3 w-3 rounded-full bg-[#febc2e]" />
          <span className="h-3 w-3 rounded-full bg-[#28c840]" />
        </div>
        <div className="text-xs font-semibold text-white/60">cinto-terminal</div>
      </div>

      <div className="relative space-y-3 px-6 py-6 font-mono text-sm text-white">
        <div className="text-[#5ED7D2] font-semibold">[C] <span className="text-white">Cinto</span> chat</div>
        <div className="text-xs text-white/60">model qwen3.5-9b   think medium   endpoint http://127.0.0.1:1234</div>

        <div className="border-b border-white/10 pb-4">
          <div className="text-[#5ED7D2] font-semibold">SYS <span className="text-white font-normal">Ready</span></div>
          <div className="text-white/80">Cinto is ready. Type a request, /tools, /todos, /prompt, /settings, /clear, or /quit.</div>
        </div>

        <div className="px-3 py-3 text-xs text-white/70 bg-[#11161d]/80 rounded border border-white/10">
          <span className="text-[#7FE7E2]">►</span> ready <span className="mx-2">·</span> - ago <span className="mx-2">·</span> ctx [ ██████████ ] 2035/262144 (1%) <span className="mx-2">·</span> Keys F2 settings F3 sidebar F4 header PgUp/PgDn scroll Ctrl-C quit
        </div>

        <div className="space-y-2">
          {OUTPUT_LINES.map((line, index) => (
            <motion.div
              key={line}
              className="text-white/70"
              initial={{ opacity: 0, y: 8 }}
              animate={index < visibleLines ? { opacity: 1, y: 0 } : { opacity: 0, y: 8 }}
              transition={{ duration: 0.3, delay: index * 0.05 }}
            >
              {line}
            </motion.div>
          ))}
        </div>

        <div className="rounded border border-white/10 bg-[#11161d]/95 p-3">
          <div className="text-xs text-[#7FE7E2]">Input</div>
          <div className="mt-2 flex items-center gap-2 rounded border border-white/10 bg-[#0c1218] px-3 py-2 text-white">
            <span className="text-[#5ED7D2]">{'>'}</span>
            <span className="flex-1 break-words min-h-[1rem]">{typed}</span>
            {!typingDone && <Cursor />}
            {typingDone && <div className="h-4 w-3 rounded bg-[#7FE7E2] opacity-80" />}
          </div>
        </div>
      </div>
    </div>
  );
}
