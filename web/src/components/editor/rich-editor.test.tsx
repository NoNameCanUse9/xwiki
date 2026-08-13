import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import RichEditor from "./rich-editor";

// jsdom has no layout engine: ProseMirror calls getClientRects()/getBoundingClientRect()
// on Range objects while scrolling the selection into view after DOM changes.
const emptyRect = () => ({
  top: 0,
  bottom: 0,
  left: 0,
  right: 0,
  width: 0,
  height: 0,
  x: 0,
  y: 0,
  toJSON: () => ({}),
});
if (typeof Range.prototype.getClientRects !== "function") {
  Object.defineProperty(Range.prototype, "getClientRects", {
    configurable: true,
    value: () => [],
  });
}
if (typeof Range.prototype.getBoundingClientRect !== "function") {
  Object.defineProperty(Range.prototype, "getBoundingClientRect", {
    configurable: true,
    value: emptyRect,
  });
}
if (typeof document.elementFromPoint !== "function") {
  document.elementFromPoint = () => null;
}

describe("RichEditor", () => {
  it("renders an editable textbox for the initial markdown content", () => {
    render(<RichEditor initialMarkdown="# Hello" onChange={vi.fn()} />);
    expect(screen.getByRole("textbox")).toBeInTheDocument();
  });

  it("calls onChange when the content changes", async () => {
    const onChange = vi.fn();
    render(<RichEditor initialMarkdown="" onChange={onChange} />);
    const editor = screen.getByRole("textbox");
    await userEvent.type(editor, "Hello");
    expect(onChange).toHaveBeenCalledWith("Hello");
  });

  it("does not call onChange on mount", () => {
    const onChange = vi.fn();
    render(<RichEditor initialMarkdown="# Hello" onChange={onChange} />);
    expect(onChange).not.toHaveBeenCalled();
  });

  it("is not editable when readOnly", () => {
    const { rerender } = render(<RichEditor initialMarkdown="" onChange={vi.fn()} readOnly />);
    expect(screen.getByRole("textbox")).toHaveAttribute("contenteditable", "false");
    rerender(<RichEditor initialMarkdown="" onChange={vi.fn()} readOnly={false} />);
    expect(screen.getByRole("textbox")).toHaveAttribute("contenteditable", "true");
  });

  it("syntax-highlights fenced json code blocks", async () => {
    render(
      <RichEditor
        initialMarkdown={'```json\n{"cabinet_no": "LB1830502359"}\n```'}
        onChange={vi.fn()}
      />,
    );
    // lowlight decorations wrap tokens in hljs-* spans.
    const attr = await screen.findByText('"cabinet_no"');
    expect(attr.className).toContain("hljs-attr");
  });
});

describe("no persistent toolbar", () => {
  it("renders no formatting toolbar buttons by default (Notion-style)", () => {
    render(<RichEditor initialMarkdown="" onChange={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "加粗" })).toBeNull();
    expect(screen.queryByRole("button", { name: "插入表格" })).toBeNull();
    expect(screen.queryByRole("button", { name: "上移块" })).toBeNull();
  });

  it("exposes formatting through the bubble menu on selection", async () => {
    render(<RichEditor initialMarkdown="hello world" onChange={vi.fn()} />);
    const editable = screen.getByRole("textbox");
    await userEvent.click(editable);

    const range = document.createRange();
    range.selectNodeContents(editable);
    const sel = window.getSelection();
    sel?.removeAllRanges();
    sel?.addRange(range);
    fireEvent.mouseUp(editable);

    // The bubble menu is portaled into document.body once the selection is
    // non-empty. Poll generously: the tippy/Floating-UI show path is
    // asynchronous under jsdom and its debounce is not under our control.
    await vi.waitFor(
      () => {
        expect(screen.getAllByRole("button", { name: "加粗" }).length).toBeGreaterThanOrEqual(1);
      },
      { timeout: 5000, interval: 100 },
    );
  });
});

// The block-level operations live in the "/" menu now (no toolbar). They
// act on the block under the caret, so the caret sits at the start of the
// second paragraph in these tests.
function placeCaretAtStart(editable: HTMLElement, paraIndex: number) {
  const paragraphs = editable.querySelectorAll("p");
  const p = paragraphs[paraIndex];
  const txt = p.firstChild; // text node
  const range = document.createRange();
  range.setStart(txt ?? p, 0);
  range.collapse(true);
  const sel = window.getSelection();
  sel?.removeAllRanges();
  sel?.addRange(range);
  // Let ProseMirror's DOMObserver pick the caret up from the DOM selection.
  document.dispatchEvent(new Event("selectionchange"));
}

async function openSlashAtStart(editable: HTMLElement, paraIndex: number) {
  await userEvent.click(editable);
  placeCaretAtStart(editable, paraIndex);
  // ProseMirror syncs DOM selection to its state via a 50ms selectionchange
  // debounce; give it room before typing the "/".
  await new Promise((r) => setTimeout(r, 150));
  await userEvent.keyboard("/");
}

describe("block operations (via slash menu)", () => {
  it("moves the current block up", async () => {
    const onChange = vi.fn();
    render(<RichEditor initialMarkdown={"para one\n\npara two"} onChange={onChange} />);
    const editable = screen.getByRole("textbox");
    // 光标移到第二段开头，敲 / 弹出菜单，选择“上移块”
    await openSlashAtStart(editable as HTMLElement, 1);
    await userEvent.click(await screen.findByText("上移块"));
    await vi.waitFor(() => {
      const last = onChange.mock.calls.at(-1)?.[0] as string;
      expect(last.indexOf("para two")).toBeLessThan(last.indexOf("para one"));
    });
  });

  it("deletes the current block", async () => {
    const onChange = vi.fn();
    render(<RichEditor initialMarkdown={"para one\n\npara two"} onChange={onChange} />);
    const editable = screen.getByRole("textbox");
    await openSlashAtStart(editable as HTMLElement, 1);
    await userEvent.click(await screen.findByText("删除块"));
    await vi.waitFor(() => {
      const last = onChange.mock.calls.at(-1)?.[0] as string;
      expect(last).not.toContain("para two");
      expect(last).toContain("para one");
    });
  });
});
describe("slash menu", () => {
  it("opens slash menu on / and inserts a heading", async () => {
    const onChange = vi.fn();
    render(<RichEditor initialMarkdown="" onChange={onChange} />);
    const editable = screen.getByRole("textbox");
    await userEvent.click(editable);
    await userEvent.keyboard("/");
    expect(await screen.findByText("标题 2")).toBeInTheDocument();
    await userEvent.click(screen.getByText("标题 2"));
    // The last onChange must be a level-2 heading ("##") and the typed "/"
    // must have been consumed by runSlash.
    await vi.waitFor(() =>
      expect(onChange).toHaveBeenLastCalledWith(
        expect.stringContaining("##"),
      ),
    );
    expect(onChange.mock.calls.at(-1)?.[0]).not.toContain("/");
    // 浮层关闭
    expect(screen.queryByText("标题 2")).not.toBeInTheDocument();
  });

  it("closes slash menu on Escape", async () => {
    render(<RichEditor initialMarkdown="" onChange={vi.fn()} />);
    await userEvent.click(screen.getByRole("textbox"));
    await userEvent.keyboard("/");
    expect(await screen.findByText("标题 1")).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByText("标题 1")).not.toBeInTheDocument();
  });

  it("does not open the slash menu when / is typed inside text or code", async () => {
    const onChange = vi.fn();
    render(<RichEditor initialMarkdown="" onChange={onChange} />);
    const editable = screen.getByRole("textbox");
    await userEvent.click(editable);
    await userEvent.keyboard("and/or");
    expect(screen.queryByText("标题 1")).not.toBeInTheDocument();
  });

  it("typing markdown shortcut characters does not convert the block", async () => {
    const onChange = vi.fn();
    render(<RichEditor initialMarkdown="" onChange={onChange} />);
    const editable = screen.getByRole("textbox");
    await userEvent.click(editable);
    await userEvent.keyboard("# not a heading");
    // Block shortcuts are disabled: no instant h1, text stays literal.
    expect(editable.querySelector("h1")).toBeNull();
    const last = onChange.mock.calls.at(-1)?.[0] as string;
    expect(last).toContain("# not a heading");
  });

  it("filters slash items by the query typed after /", async () => {
    render(<RichEditor initialMarkdown="" onChange={vi.fn()} />);
    const editable = screen.getByRole("textbox");
    await userEvent.click(editable);
    await userEvent.keyboard("/#");
    // "#" matches the heading hints (标题 1/2/3); everything else is filtered out.
    expect(await screen.findByText("标题 2")).toBeInTheDocument();
    expect(screen.queryByText("引用")).not.toBeInTheDocument();
    expect(screen.queryByText("无序列表")).not.toBeInTheDocument();
    expect(screen.queryByText("插入表格")).not.toBeInTheDocument();
  });

  it("shows a no-match state when the query matches nothing", async () => {
    render(<RichEditor initialMarkdown="" onChange={vi.fn()} />);
    const editable = screen.getByRole("textbox");
    await userEvent.click(editable);
    await userEvent.keyboard("/zzz");
    expect(await screen.findByText("无匹配项")).toBeInTheDocument();
  });

  it("consumes the slash and the query text when a command runs", async () => {
    const onChange = vi.fn();
    render(<RichEditor initialMarkdown="" onChange={onChange} />);
    const editable = screen.getByRole("textbox");
    await userEvent.click(editable);
    await userEvent.keyboard("/#");
    await userEvent.click(await screen.findByText("标题 2"));
    await vi.waitFor(() =>
      expect(onChange).toHaveBeenLastCalledWith(
        expect.stringContaining("##"),
      ),
    );
    // Both the "/" and the typed "#" must be gone.
    expect(onChange.mock.calls.at(-1)?.[0]).not.toContain("/#");
    expect(onChange.mock.calls.at(-1)?.[0]).not.toContain("/");
  });
});
