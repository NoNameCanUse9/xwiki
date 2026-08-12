# 编辑器增强（Notion 式编辑体验）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 将 docs-viewer 的裸 textarea 替换为 Notion 式所见即所得编辑器（工具栏 + slash 菜单 + 快捷键 + 块级操作），并补齐新建/删除/重命名文件的前端入口；Markdown 存取无损，正文仍只存 Git。

**Architecture:** 前端引入 Tiptap v2（ProseMirror）——`@tiptap/react` + StarterKit + Placeholder + Link + Image；Markdown 桥接用 `tiptap-markdown` 扩展（`editor.getMarkdown()` / `setContent(md)`）。加载路径：后端已有 `format=html`（goldmark）用于编辑初始渲染，**保存时取 markdown** 走现有 changeset（409 冲突、幂等全部复用）。新建/删除/重命名复用现有 ChangeSet API（create/delete/move op）。编辑器组件 `components/editor/rich-editor.tsx` 独立封装（props: initialMarkdown / onChange(md) / readOnly），docs-viewer 集成替换 textarea。

**Tech Stack:** @tiptap/react / @tiptap/pm / @tiptap/starter-kit / @tiptap/extension-placeholder / @tiptap/extension-link / @tiptap/extension-image / tiptap-markdown；现有 React19/Vite/zod/sonner；无后端改动（除可选的文件操作测试断言）。

---

## 0. 范围

**做：**

- 依赖：tiptap 全家桶 + tiptap-markdown（md 序列化/解析）
- `components/editor/rich-editor.tsx`：编辑器组件（工具栏：加粗/斜体/删除线/标题 H1-H3/引用/代码块/无序/有序列表/链接/图片/撤销重做；slash 菜单 `/` 触发；placeholder；只读模式）
- docs-viewer 集成：编辑模式 = RichEditor；保存 = `getMarkdown()` → changeset（保留 base_revision 冲突处理）；Cmd/Ctrl+S 快捷键保存；离开编辑未保存提示（confirm）
- 文档树文件操作：新建页面（输入路径 → create changeset）、删除（confirm → delete）、重命名（输入新名 → move）——全部走现有 changesets API
- 图片粘贴：粘贴图片 → base64（≤ 2 MiB）→ 以 `data:image/...;base64,` 插入编辑器（MVP 存内联，不走附件上传）
- 测试：rich-editor 渲染/工具栏/内容变化回调；docs-viewer 保存流（mock api）；文件操作（新建/删除/重命名调用断言）；全部 vitest + RTL
- 文档：development.md 前端章节补编辑器说明；api.md 无变化（复用既有端点）

**不做：** 拖拽块排序（Tiptap 需额外扩展，后续）、database/embed/协同、图片上传到 Git（MVP 内联 base64）、反向链接、md 语法高亮。

**验收：**

0. **强制提交（硬性）**：任何页面修改保存后必须产生一个新 commit——保存成功 = revision 前进（每次 +1）；保存失败则内容不丢失（错误提示 + 草稿保留在编辑器中，可重试）。禁止任何"修改了内容但没有 commit"的路径。
2. 新建页面出现在文档树并可编辑；删除后树刷新且页面 404；重命名后旧路径 404 新路径可读。
3. 编辑中有未保存修改时离开页面有确认提示；Cmd/Ctrl+S 保存成功 toast。

## 1. 文件结构

```text
web/src/
├── components/editor/
│   ├── rich-editor.tsx        （新：编辑器 + 工具栏 + slash）
│   ├── rich-editor.test.tsx   （新）
│   └── file-actions.tsx       （新：新建/删除/重命名 UI 组件）
├── routes/docs-viewer.tsx     （修改：RichEditor 替换 textarea；文件操作接入树）
├── routes/docs-viewer.test.tsx（修改：保存流断言更新）
└── package.json               （修改：tiptap 依赖）
doc/development.md             （修改：编辑器说明）
```

## 2. 任务清单（严格 TDD，每任务独立提交）

### Task 1: 安装依赖 + RichEditor 骨架

**Files:**
- Modify: `web/package.json`
- Create: `web/src/components/editor/rich-editor.tsx`
- Test: `web/src/components/editor/rich-editor.test.tsx`

- [x] **Step 1: 安装依赖**

```bash
cd web
npm install @tiptap/react @tiptap/pm @tiptap/starter-kit @tiptap/extension-placeholder @tiptap/extension-link @tiptap/extension-image @tiptap/extension-table @tiptap/extension-table-row @tiptap/extension-table-header @tiptap/extension-table-cell @tiptap/extension-bubble-menu tiptap-markdown mermaid katex highlight.js tiptap-extension-drag-handle
```

- [x] **Step 2: 写失败测试**（组件渲染 markdown 初始内容 + 触发 onChange）

```tsx
// rich-editor.test.tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import RichEditor from "./rich-editor";

describe("RichEditor", () => {
  it("renders initial markdown as content", () => {
    render(<RichEditor initialMarkdown="# Hello\n\nworld" onChange={vi.fn()} />);
    expect(screen.getByRole("textbox")).toBeInTheDocument();
  });

  it("calls onChange with markdown when content changes", async () => {
    const onChange = vi.fn();
    render(<RichEditor initialMarkdown="# Hello" onChange={onChange} />);
    const editor = screen.getByRole("textbox");
    await fireEvent.focus(editor);
    await fireEvent.input(editor, { target: { textContent: "# Hello\n\nnew" } });
    // Tiptap 内容变化通过 transaction 触发；用 editor 实例事件模拟
    expect(onChange).toHaveBeenCalled();
  });
});
```

- [x] **Step 3: 运行确认失败**

```bash
npx vitest run src/components/editor/rich-editor.test.tsx
```
Expected: FAIL（组件不存在）

- [x] **Step 4: 实现最小组件**（含 tiptap-markdown 桥接与基础工具栏容器）

```tsx
// rich-editor.tsx
import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Placeholder from "@tiptap/extension-placeholder";
import Link from "@tiptap/extension-link";
import Image from "@tiptap/extension-image";
import Table from "@tiptap/extension-table";
import TableRow from "@tiptap/extension-table-row";
import TableHeader from "@tiptap/extension-table-header";
import TableCell from "@tiptap/extension-table-cell";
import { Markdown } from "tiptap-markdown";

interface Props {
  initialMarkdown: string;
  onChange: (markdown: string) => void;
  readOnly?: boolean;
}

export default function RichEditor({ initialMarkdown, onChange, readOnly }: Props) {
  const editor = useEditor({
    extensions: [
      StarterKit,
      Placeholder.configure({ placeholder: "输入 / 唤起命令菜单…" }),
      Link.configure({ openOnClick: false }),
      Image,
      Table.configure({ resizable: true }),
      TableRow,
      TableHeader,
      TableCell,
      Markdown,
    ],
    content: initialMarkdown, // tiptap-markdown 解析 md
    editable: !readOnly,
    editorProps: { attributes: { class: "prose-xwiki focus:outline-none min-h-[30rem]" } },
    onUpdate: ({ editor }) => onChange(editor.storage.markdown.getMarkdown()),
  });
  return <EditorContent editor={editor} />;
}
```

- [x] **Step 5: 运行确认通过**

```bash
npx vitest run src/components/editor/rich-editor.test.tsx
```
Expected: PASS

- [x] **Step 6: 提交**

```bash
git add web/package.json web/package-lock.json web/src/components/editor/rich-editor.tsx web/src/components/editor/rich-editor.test.tsx
git commit -m "feat(editor): tiptap rich editor with markdown bridge"
```

### Task 2: 工具栏（格式操作）

**Files:**
- Modify: `web/src/components/editor/rich-editor.tsx`
- Test: `web/src/components/editor/rich-editor.test.tsx`

- [x] **Step 1: 写失败测试**（工具栏按钮存在且点击触发命令）

```tsx
it("renders toolbar buttons", () => {
  render(<RichEditor initialMarkdown="" onChange={vi.fn()} />);
  expect(screen.getByRole("button", { name: "加粗" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "标题 2" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "代码块" })).toBeInTheDocument();
});
```

- [x] **Step 2: 运行确认失败** → **Step 3: 实现工具栏**

```tsx
// rich-editor.tsx 内新增 Toolbar 组件
function Toolbar({ editor }: { editor: Editor | null }) {
  if (!editor) return null;
  const btn = (label: string, active: boolean, onClick: () => void, icon: string) => (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={`rounded-sm px-2 py-1 font-mono text-xs ${active ? "bg-[var(--color-surface-accent)] text-[var(--color-accent)]" : "text-[var(--color-ink-2)] hover:bg-[var(--color-surface-accent)]"}`}
    >
      {icon}
    </button>
  );
  return (
    <div className="flex flex-wrap items-center gap-1 border-b border-[var(--color-rule)] px-2 py-1.5">
      {btn("加粗", editor.isActive("bold"), () => editor.chain().focus().toggleBold().run(), "B")}
      {btn("斜体", editor.isActive("italic"), () => editor.chain().focus().toggleItalic().run(), "I")}
      {btn("删除线", editor.isActive("strike"), () => editor.chain().focus().toggleStrike().run(), "S̶")}
      <span className="mx-1 h-4 w-px bg-[var(--color-rule)]" />
      {btn("标题 1", editor.isActive("heading", { level: 1 }), () => editor.chain().focus().toggleHeading({ level: 1 }).run(), "H1")}
      {btn("标题 2", editor.isActive("heading", { level: 2 }), () => editor.chain().focus().toggleHeading({ level: 2 }).run(), "H2")}
      {btn("标题 3", editor.isActive("heading", { level: 3 }), () => editor.chain().focus().toggleHeading({ level: 3 }).run(), "H3")}
      {btn("引用", editor.isActive("blockquote"), () => editor.chain().focus().toggleBlockquote().run(), "❝")}
      {btn("插入表格", editor.isActive("table"), () => editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run(), "▦")}
      {btn("代码块", editor.isActive("codeBlock"), () => editor.chain().focus().toggleCodeBlock().run(), "</>")}
      <span className="mx-1 h-4 w-px bg-[var(--color-rule)]" />
      {btn("无序列表", editor.isActive("bulletList"), () => editor.chain().focus().toggleBulletList().run(), "•≡")}
      {btn("插入折叠块", false, () => editor.chain().focus().insertContent({ type: "paragraph", content: [{ type: "text", text: ":::details 标题\n\n内容\n:::" }] }).run(), "▸≡")}
      {btn("有序列表", editor.isActive("orderedList"), () => editor.chain().focus().toggleOrderedList().run(), "1≡")}
      <span className="mx-1 h-4 w-px bg-[var(--color-rule)]" />
      {btn("撤销", false, () => editor.chain().focus().undo().run(), "↶")}
      {btn("重做", false, () => editor.chain().focus().redo().run(), "↷")}
    </div>
  );
}
// RichEditor 返回 <div className="hairline-panel overflow-hidden"><Toolbar editor={editor} /><EditorContent editor={editor} className="px-4 py-3" /></div>
```

- [x] **Step 4: 运行确认通过** → **Step 5: 提交**（`feat(editor): toolbar with formatting commands`）

### Task 3: Slash 菜单

**Files:**
- Modify: `web/src/components/editor/rich-editor.tsx`
- Test: `web/src/components/editor/rich-editor.test.tsx`

- [x] **Step 1: 失败测试**（输入 `/` 出现菜单，点击"标题 2"插入）

```tsx
it("opens slash menu on / and inserts a heading", async () => {
  const user = userEvent.setup();
  render(<RichEditor initialMarkdown="" onChange={vi.fn()} />);
  const editor = screen.getByRole("textbox");
  await user.type(editor, "/");
  expect(await screen.findByText("标题 2")).toBeInTheDocument();
  await user.click(screen.getByText("标题 2"));
  // 菜单关闭且内容含 h2（通过 onChange 捕获）
});
```

- [x] **Step 2: 确认失败** → **Step 3: 实现**（keydown 监听 `/` → 浮层；菜单项：标题1-3/引用/代码块/无序列表/有序列表/链接；Esc/点击外部关闭；选择后执行命令并删除已输入的 `/`）

```tsx
// rich-editor.tsx 内
const [slashOpen, setSlashOpen] = useState(false);
const [slashPos, setSlashPos] = useState(0);

// editorProps.handleKeyDown
handleKeyDown: (view, event) => {
  if (event.key === "/" && !readOnly) {
    setSlashPos(view.state.selection.from - 1);
    setSlashOpen(true);
    return false; // 让 / 正常输入
  }
  if (event.key === "Escape") setSlashOpen(false);
  return false;
},
// 菜单渲染：slashOpen && 固定定位在编辑器上方；onMouseDown 阻止失焦
// 菜单项执行: editor.chain().focus().deleteRange({from: slashPos, to: slashPos + 1}).toggleHeading({level: 2}).run()
```

- [x] **Step 4: 确认通过** → **Step 5: 提交**（`feat(editor): slash command menu`）

### Task 3a: 选中文字浮动工具条（Bubble Menu）

**Files:**
- Modify: `web/src/components/editor/rich-editor.tsx`
- Test: `web/src/components/editor/rich-editor.test.tsx`

- [x] **Step 1: 失败测试**（选中文字 → 浮层出现（加粗/斜体/链接/代码/清除格式按钮）；点击加粗 → 文本包 strong）

```tsx
it("shows bubble menu on text selection", async () => {
  render(<RichEditor initialMarkdown="hello world" onChange={vi.fn()} />);
  const editor = screen.getByRole("textbox");
  await user.pointer([{ keys: "[MouseLeft>]", target: editor }, { keys: "/" }]); // 选中部分文本（RTL 用 Selection API 或 fireEvent.select）
  // 断言屏幕出现 aria-label="链接" 的按钮
  await user.click(screen.getByRole("button", { name: "加粗" }));
  // onChange 回调的 md 含 **选中文本**
});
```

- [x] **Step 2: 确认失败** → **Step 3: 实现**（`@tiptap/extension-bubble-menu` 的 BubbleMenu 组件包裹工具栏子集：加粗/斜体/删除线/行内代码/链接/清除格式；`editor.chain().focus().extendMarkRange('bold').toggleBold().run()`；链接按钮用 window.prompt 输入 URL；清除格式 `unsetAllMarks().clearNodes()`）→ **Step 4: 通过** → **Step 5: 提交**（`feat(editor): bubble menu on selection`）

### Task 3b: 块拖拽手柄（Drag Handle）

**Files:**
- Modify: `web/src/components/editor/rich-editor.tsx`
- Test: `web/src/components/editor/rich-editor.test.tsx`

- [x] **Step 1: 失败测试**（渲染后编辑器存在；拖拽手柄扩展已注册——通过拖动改变块顺序后 onChange md 顺序变化；若社区包与 Tiptap 版本不兼容则回退：手柄点击弹出块操作菜单（删除/复制/上移/下移），不做真拖拽）

```tsx
it("registers drag handle and moves a block", async () => {
  // 若 DragHandle 扩展注册成功：两个段落块，拖第二个到第一个前 -> onChange 顺序反转
  // 回退断言：手柄按钮存在且点击弹出"删除块"菜单
});
```

- [x] **Step 2: 确认失败** → **Step 3: 实现**（优先 `tiptap-extension-drag-handle`（v2 兼容版）：`DragHandle.configure({ dragHandleWidth: 20 })` + CSS 手柄样式；若版本冲突（Step 2 报错）→ 回退方案：自写 NodeView 手柄（点击 → 浮动菜单：上移/下移/复制/删除）→ **Step 4: 通过** → **Step 5: 提交**（`feat(editor): block drag handle`）

### Task 3c: Cmd+K 命令面板（全局跳转/新建）

**Files:**
- Create: `web/src/components/editor/command-palette.tsx` + test
- Modify: `web/src/routes/docs-viewer.tsx`（挂载）
- Modify: `web/src/lib/api/search.ts`（复用 searchProject）

- [x] **Step 1: 失败测试**（Cmd/Ctrl+K → 面板打开；输入关键词 → 展示搜索结果；点击 → navigate 到文档；Esc 关闭）

```tsx
it("opens command palette with Cmd+K and navigates", async () => {
  // mock searchProject 返回 [{path:"docs/a.md"}]
  render(<DocsViewerPage />); // 或独立组件测试
  await user.keyboard("{Control>}k{/Control}");
  expect(await screen.findByPlaceholderText("搜索或跳转…")).toBeInTheDocument();
  await user.type(activeElement, "guide");
  await user.click(await screen.findByText("docs/a.md"));
  // 断言 navigate 到 /projects/prj_1/docs/docs/a.md
});
```

- [x] **Step 2: 确认失败** → **Step 3: 实现**（command-palette.tsx：全局 keydown（Cmd/Ctrl+K）→ Dialog 浮层（复用现有 dialog.tsx）：输入框（自动聚焦）+ 结果列表（searchProject 防抖 200ms）+ 回车选第一项 + Esc/遮罩关闭；面板内"新建页面"入口（跳转新建表单））→ **Step 4: 通过** → **Step 5: 提交**（`feat(editor): cmd+k command palette`）

### Task 4: docs-viewer 集成（保存流 + 快捷键 + 未保存提示）

**Files:**
- Modify: `web/src/routes/docs-viewer.tsx`
- Modify: `web/src/routes/docs-viewer.test.tsx`

- [x] **Step 1: 失败测试**（编辑保存走 getMarkdown 值；Cmd+S；离开确认）

```tsx
it("saves via rich editor markdown and Cmd+S", async () => {
  // mock getPage(raw) -> "# Guide"; submitChangeset 断言 content 为编辑器输出
  // renderPage("/projects/prj_1/docs/guide.md"); 点击编辑
  // 编辑器输入 "## New"; 按 Cmd+S; 断言 submitChangeset 被调用且 content 含 "## New"
});
it("saving always advances the revision (forced commit)", async () => {
  // mock getRevision 依调用次序返回 r1, r1, r2（保存前取 base，保存后刷新取新值）
  // mock submitChangeset 成功
  // 编辑 -> 保存 -> 断言 submitChangeset 调用 1 次，且保存后 revision 与保存前不同
  // 再编辑 -> 保存 -> 断言 submitChangeset 调用 2 次（每次保存都提交，绝不静默跳过）
});

it("prompts before leaving with unsaved changes", async () => {
  // window.confirm mock; 编辑后点返回 workspace; 断言 confirm 被调用
});
```

- [x] **Step 2: 确认失败** → **Step 3: 实现**

```tsx
// docs-viewer.tsx 编辑区替换
{editing && (
  <div className="space-y-3">
    <div className="flex items-center justify-between">
      <p className="mono-label text-[var(--color-ink-3)]">editing · {filePath}</p>
      <div className="flex gap-2">
        <Button variant="ghost" size="sm" onClick={() => setEditing(false)}>取消</Button>
        <Button size="sm" onClick={() => void saveEdit()} disabled={saving}>
          {saving ? "保存中…" : "保存"}
        </Button>
      </div>
    </div>
    {rawQuery.isLoading ? (
      <p className="mono-label text-[var(--color-ink-3)]">loading…</p>
    ) : (
      <RichEditor
        initialMarkdown={draft}
        onChange={(md) => { setDraft(md); setDirty(true); }}
        readOnly={false}
      />
    )}
  </div>
)}
// saveEdit 增强：catch 时保持 editing=true + draft 不清空（内容不丢失），toast 错误可重试
// 成功路径：toast + editing=false + invalidate（不变）
// 新增：dirty state + 全局 keydown 监听 Cmd/Ctrl+S（saveEdit）
// 新增：beforeunload / 导航守卫（useBlocker 或手动 confirm：编辑中且 dirty 时点其他链接先 confirm）
```

- [x] **Step 4: 确认通过** → **Step 5: 提交**（`feat(editor): wire rich editor save flow with shortcuts`）

### Task 5: 新建页面

**Files:**
- Create: `web/src/components/editor/file-actions.tsx`
- Modify: `web/src/routes/docs-viewer.tsx`
- Modify: `web/src/routes/docs-viewer.test.tsx`

- [x] **Step 1: 失败测试**

```tsx
it("creates a new page from the tree", async () => {
  // mock submitChangeset; 点击"新建页面"; 输入 docs/new.md; 提交
  // 断言 submitChangeset 以 create op + 新路径调用; 树刷新
});
```

- [x] **Step 2: 确认失败** → **Step 3: 实现**（file-actions.tsx：新建按钮 → 小表单（路径输入 + 创建）→ create changeset（base=当前 revision，content=""; message="Create <path>"）→ invalidate tree）

- [x] **Step 4: 确认通过** → **Step 5: 提交**（`feat(editor): create page entry`）

### Task 6: 删除 + 重命名

**Files:**
- Modify: `web/src/components/editor/file-actions.tsx`
- Modify: `web/src/routes/docs-viewer.tsx`
- Modify: `web/src/routes/docs-viewer.test.tsx`

- [x] **Step 1: 失败测试**（树文件行 hover 菜单：删除 → confirm → delete op；重命名 → 输入新路径 → move op + new_path）

- [x] **Step 2: 确认失败** → **Step 3: 实现**（文件行右侧"⋯"菜单或按钮：删除（confirm + delete changeset）、重命名（内联输入 + move changeset）；当前文件被删/改名后导航到 /docs）

- [x] **Step 4: 确认通过** → **Step 5: 提交**（`feat(editor): delete and rename pages`）

### Task 7: 嵌入组件渲染管线（代码高亮 + KaTeX + mermaid）

**Files:**
- Create: `web/src/components/editor/markdown-render.tsx`（渲染增强封装）
- Modify: `web/src/routes/docs-viewer.tsx`（渲染走新封装）
- Test: `web/src/components/editor/markdown-render.test.tsx`

- [x] **Step 1: 失败测试**

```tsx
// markdown-render.test.tsx：给定含 ```js 代码块 / \(a^2\) / ```mermaid 的 HTML，
// 渲染后断言：code 元素带 hljs 类、katex 公式被替换为 .katex 元素、mermaid 块被 svg 替换
```

- [x] **Step 2: 确认失败** → **Step 3: 实现**（markdown-render.tsx 封装 useEffect 管线）

```tsx
// markdown-render.tsx
import { useEffect, useRef } from "react";
import hljs from "highlight.js";
import katex from "katex";
import mermaid from "mermaid";
import "highlight.js/styles/github-dark.css";
import "katex/dist/katex.min.css";

mermaid.initialize({ startOnLoad: false, theme: "neutral", securityLevel: "strict" });

export function enhanceRenderedMarkdown(root: HTMLElement) {
  // 1. 代码高亮：pre code[class*="language-"] -> hljs.highlightElement
  root.querySelectorAll("pre code[class*='language-']").forEach((el) => {
    if (el.textContent && !el.classList.contains("hljs")) hljs.highlightElement(el as HTMLElement);
  });
  // 2. mermaid：pre code.language-mermaid -> mermaid.render -> 替换
  root.querySelectorAll("pre code.language-mermaid").forEach(async (el) => {
    const pre = el.parentElement;
    if (!pre || pre.dataset.rendered) return;
    pre.dataset.rendered = "1";
    try {
      const { svg } = await mermaid.render("mmd-" + Math.random().toString(36).slice(2), el.textContent ?? "");
      pre.outerHTML = svg;
    } catch { /* 保留原文 */ }
  });
  // 3. KaTeX：文本节点中的 \(...\) 与 \[...\]（递归遍历，行内/块级）
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const nodes: Text[] = [];
  while (walker.nextNode()) nodes.push(walker.currentNode as Text);
  nodes.forEach((node) => {
    const text = node.textContent ?? "";
    const inline = /\\\((.+?)\\\)/gs;
    const block = /\\\[(.+?)\\]/gs;
    if (inline.test(text) || block.test(text)) {
      const span = document.createElement("span");
      span.innerHTML = renderMath(text); // 公式段用 katex.renderToString(公式, {throwOnError:false})
      node.replaceWith(span);
    }
  });
}

// docs-viewer.tsx：html 注入后调用 enhanceRenderedMarkdown(articleRef.current)
```

- [x] **Step 4: 确认通过** → **Step 5: 提交**（`feat(markdown): highlight math and mermaid rendering`）

### Task 7a: Special blocks 容器语法（:::info/warning/danger/details）

**Files:**
- Create: `internal/markdownx/blocks.go` + `internal/markdownx/blocks_test.go`（goldmark 自定义块扩展）
- Modify: `internal/httpapi/handlers/docs.go`（挂载扩展）
- Modify: `web/src/index.css`（容器样式）

- [x] **Step 1: 失败测试**

```go
// blocks_test.go
func TestSpecialBlocks(t *testing.T) {
	// 输入: ":::warning\n小心\n:::" -> HTML 含 <div class="admonition warning">
	// ":::details 更多\n内容\n:::" -> <details><summary>更多</summary>内容</details>
	// ":::info"/":::danger" 同理; 未闭合容器原样输出
}
```

- [x] **Step 2: 确认失败** → **Step 3: 实现**（goldmark 自定义 BlockParser + Renderer：`:::kind [标题]` 开块、`:::` 结束；kind ∈ info/warning/danger/details；渲染 `<div class="admonition <kind>">` 或 `<details><summary>`；内部按普通 markdown 继续解析——用 ast.CustomBlock + 子解析）

```go
// internal/markdownx/blocks.go（骨架）
package markdownx

import (
	"github.com/yuin/goldmark/ast"
	"github.com/yuin/goldmark/parser"
	"github.com/yuin/goldmark/renderer"
	"github.com/yuin/goldmark/util"
)

// Admonition 节点 + parser：行首 ^:::(\w+)(?:\s+(.*))?$ 开块；^:::$ 闭块
// renderer：kind=details -> <details><summary>标题</summary>；其余 -> <div class="admonition kind">
// 实现要点：parser.BlockParser 接口（Trigger/Open/Continue/Close），内容复用 paragraph/child 解析
```

- [x] **Step 4: 确认通过** → **Step 5: 提交**（`feat(markdown): admonition and details blocks`）

### Task 7b: 内部 wiki 链接 + 任务列表样式

**Files:**
- Modify: `internal/httpapi/handlers/docs.go`（链接重写）
- Modify: `web/src/index.css`（task list 样式）
- Modify: `web/src/routes/docs-viewer.tsx`（wiki 链接点击拦截）
- Test: `internal/server/docs_test.go`

- [x] **Step 1: 失败测试**

```go
// docs_test.go：写入 "[[docs/guide.md]] 与 [[docs/guide.md|指南]]"
// format=html 断言 href="/api/.../docs/pages/docs/guide.md"（相对 wiki 链接解析）
```

- [x] **Step 2: 确认失败** → **Step 3: 实现**
  - 后端：goldmark AST 后处理（ast.Walk 找 Link 节点，text 为 `[[path]]` 或 `[[path|label]]` 模式 → 改写 href 为项目内相对路径 `/projects/{id}/docs/{path}` 并在链接上加 `data-wiki-link`）；纯文本 `[[...]]`（非链接上下文）转 Link 节点
  - 前端：docs-viewer 对 `data-wiki-link` 的 `<a>` 点击拦截（preventDefault → navigate），支持相对路径解析（`./x.md` → 当前目录）
  - CSS：`.prose-xwiki ul:has(> li > input[type=checkbox])` 去列表符号 + checkbox 样式
- [x] **Step 4: 确认通过** → **Step 5: 提交**（`feat(wikilinks): internal page links and task list styles`）

### Task 7c: 图片粘贴（内联 base64）

**Files:**
- Modify: `web/src/components/editor/rich-editor.tsx`
- Test: `web/src/components/editor/rich-editor.test.tsx`

- [x] **Step 1: 失败测试**（paste 事件带 image file → 编辑器插入 image 节点且 src 以 data:image 开头）

```tsx
it("pastes an image as inline base64", async () => {
  // 构造 DataTransfer 含 png file; fireEvent.paste(editor, { clipboardData })
  // 断言编辑器 HTML 含 <img src="data:image/png;base64,...">
});
```

- [x] **Step 2: 确认失败** → **Step 3: 实现**（editorProps.handlePaste：扫描 items 找 image → FileReader → base64（> 2 MiB 拒绝并 toast）→ `editor.chain().focus().setImage({ src }).run()`）

- [x] **Step 4: 确认通过** → **Step 5: 提交**（`feat(editor): paste image as inline base64`）

### Task 8: 文档 + 全量验收

**Files:**
- Modify: `doc/development.md`

- [x] **Step 1: development.md 前端章节补编辑器说明**（Tiptap 架构、md 桥接、保存流、文件操作入口）

- [x] **Step 2: 全量验证**

```bash
cd web && npx vitest run        # 全部前端测试
cd .. && go test ./... -count=1 && go vet ./...
npm run build && go build -o xwiki ./cmd/xwiki && git restore web/dist/index.html
```

- [x] **Step 3: 手工验收**（浏览器）：
  1. 打开文档 → 编辑 → 加粗/标题/列表/slash 菜单 → Cmd+S → 刷新内容一致
  2. 新建 `docs/hello.md` → 树出现 → 打开编辑保存
  3. 重命名 `docs/hello.md` → `docs/world.md` → 旧路径 404
  4. 删除 `docs/world.md` → 树移除
  5. 未保存修改离开 → confirm 提示
  6. **强制提交**：编辑保存 3 次 → commits 列表恰好新增 3 条（revision 前进 3 次）

- [x] **Step 4: 提交**（`docs: editor documentation`）

## 3. 风险

- **tiptap-markdown 兼容性**：若与 Tiptap v2 版本不兼容（解析/序列化错误），回退方案：加载用后端 `format=html` 直接 `setContent(html)`（goldmark 输出与 StarterKit schema 对齐），保存用自写轻量 md 序列化（任务内给出 fallback 代码路径）；决定点：Task 1 Step 5 测试通过与否。
- **Slash 菜单定位**：MVP 固定浮层（编辑器顶部），不做光标跟随定位。
- **图片内联**：大图膨胀 md 体积（2 MiB 上限 + 单文档 5 MiB 写入限制已有兜底）。
- **未保存提示**：用 `window.confirm` + beforeunload；React Router 导航拦截用 `useBlocker`（react-router v7 支持）。

## 4. 后续候选（本次不做，另立计划）

1. 文档间链接：`[[path]]` 解析 + 点击跳转 + 反向链接
2. 大纲 TOC（标题自动生成侧栏）
3. 账户设置页（改密 UI）
4. 项目级用户权限（成员管理）
5. 导出按钮 UI（zip/bundle 下载）
6. 最近更新视图（跨项目 commit 聚合）
