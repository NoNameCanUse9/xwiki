import { describe, expect, it, vi } from "vitest";
import { enhanceRenderedMarkdown } from "./markdown-render";

// Mermaid render is async; stub it to emit a predictable svg.
vi.mock("mermaid", async (importOriginal) => {
  const actual = await importOriginal<typeof import("mermaid")>();
  return {
    ...actual,
    default: {
      ...actual.default,
      initialize: vi.fn(),
      render: vi.fn().mockResolvedValue({ svg: "<svg data-testid='mmd'>x</svg>" }),
    },
  };
});

function makeRoot(html: string): HTMLElement {
  const div = document.createElement("div");
  div.innerHTML = html;
  return div;
}

describe("enhanceRenderedMarkdown", () => {
  it("highlights code blocks", async () => {
    const root = makeRoot(
      `<pre><code class="language-js">const x = 1;</code></pre>`,
    );
    await enhanceRenderedMarkdown(root);
    expect(root.querySelector("code")?.classList.contains("hljs")).toBe(true);
  });

  it("renders mermaid blocks into svg", async () => {
    const root = makeRoot(
      `<pre><code class="language-mermaid">graph LR; A-->B</code></pre>`,
    );
    await enhanceRenderedMarkdown(root);
    expect(root.querySelector("[data-testid='mmd']")).not.toBeNull();
    expect(root.querySelector("pre")).toBeNull();
  });

  it("renders inline and block math with katex", async () => {
    const root = makeRoot(
      `<p>inline \\(a^2\\) here</p><div>\\[\\frac{1}{2}\\]</div>`,
    );
    await enhanceRenderedMarkdown(root);
    expect(root.querySelector(".katex")).not.toBeNull();
  });
});
