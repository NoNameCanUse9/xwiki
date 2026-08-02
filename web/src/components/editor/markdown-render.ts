import hljs from "highlight.js";
import katex from "katex";
import mermaid from "mermaid";
import "highlight.js/styles/github-dark.css";
import "katex/dist/katex.min.css";

mermaid.initialize({ startOnLoad: false, theme: "neutral", securityLevel: "strict" });

const INLINE_MATH = /\\\((.+?)\\\)/gs;
const BLOCK_MATH = /\\\[([\s\S]+?)\\\]/g;

/**
 * Post-processes server-rendered markdown HTML inside `root`:
 * 1. Syntax-highlights ```lang code blocks (highlight.js)
 * 2. Renders ```mermaid blocks into SVGs (mermaid.js)
 * 3. Renders \(inline\) and \[block\] math (KaTeX)
 */
export async function enhanceRenderedMarkdown(root: HTMLElement): Promise<void> {
  // 1. Code highlighting (skip mermaid blocks — they are handled next).
  root.querySelectorAll("pre code[class*='language-']").forEach((el) => {
    if (el.textContent && !el.classList.contains("language-mermaid") && !el.classList.contains("hljs")) {
      hljs.highlightElement(el as HTMLElement);
    }
  });

  // 2. Mermaid diagrams.
  const mermaidBlocks = Array.from(
    root.querySelectorAll("pre code.language-mermaid"),
  );
  for (const el of mermaidBlocks) {
    const pre = el.parentElement;
    if (!pre || pre.dataset.rendered) continue;
    pre.dataset.rendered = "1";
    try {
      const { svg } = await mermaid.render(
        "mmd-" + Math.random().toString(36).slice(2),
        el.textContent ?? "",
      );
      const wrapper = document.createElement("div");
      wrapper.className = "my-4 overflow-x-auto";
      wrapper.innerHTML = svg;
      pre.replaceWith(wrapper);
    } catch {
      // keep the raw code block on render failure
    }
  }

  // 3. KaTeX math: walk text nodes, replace math segments with spans.
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const textNodes: Text[] = [];
  while (walker.nextNode()) textNodes.push(walker.currentNode as Text);
  for (const node of textNodes) {
    const text = node.textContent ?? "";
    if (!INLINE_MATH.test(text) && !BLOCK_MATH.test(text)) continue;
    INLINE_MATH.lastIndex = 0;
    BLOCK_MATH.lastIndex = 0;
    const span = document.createElement("span");
    span.innerHTML = renderMath(text);
    node.replaceWith(span);
  }
}

function renderMath(text: string): string {
  // Pass 1: block math \[...\] (display mode).
  const blockHtml = text.replace(BLOCK_MATH, (_all, body: string) => {
    try {
      return katex.renderToString(body, { displayMode: true, throwOnError: false });
    } catch {
      return escapeHtml(_all);
    }
  });
  // Pass 2: inline math \(...\) (the KaTeX output must not contain the marker).
  return blockHtml.replace(INLINE_MATH, (_all, body: string) => {
    try {
      return katex.renderToString(body, { displayMode: false, throwOnError: false });
    } catch {
      return escapeHtml(_all);
    }
  });
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}
