import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
});

describe("toolbar", () => {
  const TOOLBAR_BUTTON_LABELS = [
    "加粗",
    "斜体",
    "删除线",
    "标题 1",
    "标题 2",
    "标题 3",
    "引用",
    "代码块",
    "插入表格",
    "无序列表",
    "有序列表",
    "插入折叠块",
    "撤销",
    "重做",
  ];

  it("renders all formatting toolbar buttons", () => {
    render(<RichEditor initialMarkdown="" onChange={vi.fn()} />);
    for (const label of TOOLBAR_BUTTON_LABELS) {
      expect(screen.getByRole("button", { name: label })).toBeInTheDocument();
    }
  });

  it("toggles the active state when clicking bold", async () => {
    const user = userEvent.setup();
    render(<RichEditor initialMarkdown="" onChange={vi.fn()} />);
    const bold = screen.getByRole("button", { name: "加粗" });
    await user.click(bold);
    // With an empty selection, toggleBold sets the stored mark, which makes
    // isActive("bold") true and flips aria-pressed on the button.
    await waitFor(() => expect(bold).toHaveAttribute("aria-pressed", "true"));
  });

  it("inserts a details block placeholder when clicking the details button", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<RichEditor initialMarkdown="" onChange={onChange} />);
    await user.click(screen.getByRole("button", { name: "插入折叠块" }));
    await waitFor(() =>
      expect(onChange).toHaveBeenCalledWith(
        expect.stringContaining(":::details"),
      ),
    );
  });

  it("does not crash when clicking any toolbar button", async () => {
    const user = userEvent.setup();
    render(<RichEditor initialMarkdown="" onChange={vi.fn()} />);
    for (const label of TOOLBAR_BUTTON_LABELS) {
      const button = screen.getByRole("button", { name: label });
      if (!button.hasAttribute("disabled")) {
        await user.click(button);
      }
    }
    expect(screen.getByRole("textbox")).toBeInTheDocument();
  });
});

describe("bubble menu", () => {
  it("shows bubble menu on text selection", async () => {
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
        expect(screen.getAllByRole("button", { name: "加粗" }).length).toBeGreaterThanOrEqual(2);
      },
      { timeout: 5000, interval: 100 },
    );
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
});

describe("block operations", () => {
  it("moves the current block up", async () => {
    const onChange = vi.fn();
    render(<RichEditor initialMarkdown={"para one\n\npara two"} onChange={onChange} />);
    const editable = screen.getByRole("textbox");
    await userEvent.click(editable);
    // 光标移到第二段末尾（jsdom 不支持 Ctrl+End 组合键）
    const paragraphs = editable.querySelectorAll("p");
    const last = paragraphs[paragraphs.length - 1];
    const txt = last.firstChild; // text node
    const range = document.createRange();
    range.setStart(txt ?? last, txt?.textContent?.length ?? 0);
    range.collapse(true);
    const sel = window.getSelection();
    sel?.removeAllRanges();
    sel?.addRange(range);
    await userEvent.click(screen.getByRole("button", { name: "上移块" }));
    await vi.waitFor(() => {
      const last = onChange.mock.calls.at(-1)?.[0] as string;
      expect(last.indexOf("para two")).toBeLessThan(last.indexOf("para one"));
    });
  });

  it("deletes the current block", async () => {
    const onChange = vi.fn();
    render(<RichEditor initialMarkdown={"para one\n\npara two"} onChange={onChange} />);
    const editable = screen.getByRole("textbox");
    await userEvent.click(editable);
    const paragraphs = editable.querySelectorAll("p");
    const last = paragraphs[paragraphs.length - 1];
    const txt = last.firstChild;
    const range = document.createRange();
    range.setStart(txt ?? last, txt?.textContent?.length ?? 0);
    range.collapse(true);
    const sel = window.getSelection();
    sel?.removeAllRanges();
    sel?.addRange(range);
    await userEvent.click(screen.getByRole("button", { name: "删除块" }));
    await vi.waitFor(() => {
      const last = onChange.mock.calls.at(-1)?.[0] as string;
      expect(last).not.toContain("para two");
      expect(last).toContain("para one");
    });
  });
});
