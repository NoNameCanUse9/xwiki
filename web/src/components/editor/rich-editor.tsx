import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  EditorContent,
  useEditor,
  useEditorState,
  type Editor,
} from "@tiptap/react";
// tiptap v3 exports the floating/bubble menu components from the `/menus`
// subpath (not from the package root), and positions them with Floating UI
// (there is no tippy.js anymore, hence `options` instead of `tippyOptions`).
import { BubbleMenu } from "@tiptap/react/menus";
import StarterKit from "@tiptap/starter-kit";
import Placeholder from "@tiptap/extension-placeholder";
import Link from "@tiptap/extension-link";
import Image from "@tiptap/extension-image";
import { Table } from "@tiptap/extension-table";
import { TableRow } from "@tiptap/extension-table-row";
import { TableHeader } from "@tiptap/extension-table-header";
import { TableCell } from "@tiptap/extension-table-cell";
import { Markdown, type MarkdownStorage } from "tiptap-markdown";
import { cn } from "../../lib/utils";

// Extension instances are module-level constants so they are not recreated
// on every render. Each editor clones them during creation, so sharing the
// instances across editor instances is safe.
const extensions = [
  StarterKit,
  Placeholder.configure({ placeholder: "输入 / 唤起命令菜单…" }),
  Link.configure({ openOnClick: false }),
  Image,
  Table.configure({ resizable: true }),
  TableRow,
  TableHeader,
  TableCell,
  Markdown,
];

export interface RichEditorProps {
  /** Markdown content used to initialize the editor (parsed by the Markdown extension). */
  initialMarkdown: string;
  /** Called with the serialized markdown whenever the document changes. */
  onChange: (markdown: string) => void;
  /** When true, the editor content is not editable. */
  readOnly?: boolean;
}

const BUTTON_BASE_CLASS =
  "rounded-sm px-2 py-1 font-mono text-xs text-[var(--color-ink-2)] transition-colors hover:bg-[var(--color-surface-accent)] disabled:pointer-events-none disabled:opacity-40";
const BUTTON_ACTIVE_CLASS =
  "bg-[var(--color-surface-accent)] text-[var(--color-accent)]";

interface ToolbarButtonProps {
  /** Accessible name shown in aria-label and title. */
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}

function ToolbarButton({
  label,
  active = false,
  disabled = false,
  onClick,
  children,
}: ToolbarButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={active}
      disabled={disabled}
      onClick={onClick}
      className={cn(BUTTON_BASE_CLASS, active && BUTTON_ACTIVE_CLASS)}
    >
      {children}
    </button>
  );
}

function ToolbarDivider() {
  return (
    <span
      aria-hidden="true"
      className="mx-1 h-4 w-px bg-[var(--color-rule)]"
    />
  );
}

interface ToolbarProps {
  editor: Editor;
}

function Toolbar({ editor }: ToolbarProps) {
  // Subscribe to editor state so the active/disabled button states stay in
  // sync with the current selection and document (tiptap v3 recommended way).
  const state = useEditorState({
    editor,
    selector: ({ editor: e }) => ({
      bold: e.isActive("bold"),
      italic: e.isActive("italic"),
      strike: e.isActive("strike"),
      heading1: e.isActive("heading", { level: 1 }),
      heading2: e.isActive("heading", { level: 2 }),
      heading3: e.isActive("heading", { level: 3 }),
      blockquote: e.isActive("blockquote"),
      codeBlock: e.isActive("codeBlock"),
      bulletList: e.isActive("bulletList"),
      orderedList: e.isActive("orderedList"),
      canUndo: e.can().undo(),
      canRedo: e.can().redo(),
      editable: e.isEditable,
    }),
  });

  // Insert a raw ":::details" block as plain text. The details extension that
  // renders it as a collapsible block is wired up in a later task.
  // Locate the block index the cursor is in (top-level blocks only).
      const currentBlockIndex = (e: Editor): number => {
        const { from } = e.state.selection;
        let idx = 0;
        e.state.doc.forEach((node, offset) => {
          if (offset + node.nodeSize <= from) idx++;
        });
        const total = (e.getJSON().content ?? []).length;
        return Math.min(idx, Math.max(total - 1, 0));
      };

      const moveBlock = (dir: -1 | 1) => {
        const json = editor.getJSON();
        const blocks = [...((json.content ?? []) as object[])];
        const idx = currentBlockIndex(editor);
        const swap = idx + dir;
        if (blocks.length < 2 || swap < 0 || swap >= blocks.length) return;
        [blocks[idx], blocks[swap]] = [blocks[swap], blocks[idx]];
        editor.chain().focus().setContent({ ...json, content: blocks }).run();
      };

      const copyBlock = () => {
        const json = editor.getJSON();
        const blocks = [...((json.content ?? []) as object[])];
        const idx = currentBlockIndex(editor);
        if (blocks.length === 0) return;
        blocks.splice(idx + 1, 0, JSON.parse(JSON.stringify(blocks[idx])));
        editor.chain().focus().setContent({ ...json, content: blocks }).run();
      };

      const deleteBlock = () => {
        const json = editor.getJSON();
        const blocks = [...((json.content ?? []) as object[])];
        const idx = currentBlockIndex(editor);
        if (blocks.length === 0) return;
        blocks.splice(idx, 1);
        if (blocks.length === 0) {
          blocks.push({ type: "paragraph" });
        }
        editor.chain().focus().setContent({ ...json, content: blocks }).run();
      };

      const insertDetailsBlock = () =>
    editor
      .chain()
      .focus()
      .insertContent({
        type: "paragraph",
        content: [{ type: "text", text: ":::details 标题\n\n内容\n:::" }],
      })
      .run();

  return (
    <div className="flex flex-wrap items-center gap-1 border-b border-[var(--color-rule)] px-2 py-1.5">
      <ToolbarButton
        label="加粗"
        active={state.bold}
        disabled={!state.editable}
        onClick={() => editor.chain().focus().toggleBold().run()}
      >
        <strong>B</strong>
      </ToolbarButton>
      <ToolbarButton
        label="斜体"
        active={state.italic}
        disabled={!state.editable}
        onClick={() => editor.chain().focus().toggleItalic().run()}
      >
        <em>I</em>
      </ToolbarButton>
      <ToolbarButton
        label="删除线"
        active={state.strike}
        disabled={!state.editable}
        onClick={() => editor.chain().focus().toggleStrike().run()}
      >
        <s>S̶</s>
      </ToolbarButton>
      <ToolbarDivider />
      <ToolbarButton
        label="标题 1"
        active={state.heading1}
        disabled={!state.editable}
        onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()}
      >
        H1
      </ToolbarButton>
      <ToolbarButton
        label="标题 2"
        active={state.heading2}
        disabled={!state.editable}
        onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
      >
        H2
      </ToolbarButton>
      <ToolbarButton
        label="标题 3"
        active={state.heading3}
        disabled={!state.editable}
        onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()}
      >
        H3
      </ToolbarButton>
      <ToolbarDivider />
      <ToolbarButton
        label="引用"
        active={state.blockquote}
        disabled={!state.editable}
        onClick={() => editor.chain().focus().toggleBlockquote().run()}
      >
        ❝
      </ToolbarButton>
      <ToolbarButton
        label="代码块"
        active={state.codeBlock}
        disabled={!state.editable}
        onClick={() => editor.chain().focus().toggleCodeBlock().run()}
      >
        {"</>"}
      </ToolbarButton>
      <ToolbarButton
        label="插入表格"
        disabled={!state.editable}
        onClick={() =>
          editor
            .chain()
            .focus()
            .insertTable({ rows: 3, cols: 3, withHeaderRow: true })
            .run()
        }
      >
        ▦
      </ToolbarButton>
      <ToolbarDivider />
      <ToolbarButton
        label="无序列表"
        active={state.bulletList}
        disabled={!state.editable}
        onClick={() => editor.chain().focus().toggleBulletList().run()}
      >
        {"•≡"}
      </ToolbarButton>
      <ToolbarButton
        label="有序列表"
        active={state.orderedList}
        disabled={!state.editable}
        onClick={() => editor.chain().focus().toggleOrderedList().run()}
      >
        {"1≡"}
      </ToolbarButton>
      <ToolbarButton
        label="插入折叠块"
        disabled={!state.editable}
        onClick={insertDetailsBlock}
      >
        {"▸≡"}
      </ToolbarButton>
      <ToolbarDivider />
      <ToolbarButton
        label="撤销"
        disabled={!state.editable || !state.canUndo}
        onClick={() => editor.chain().focus().undo().run()}
      >
        ↶
      </ToolbarButton>
      <ToolbarButton
        label="重做"
        disabled={!state.editable || !state.canRedo}
        onClick={() => editor.chain().focus().redo().run()}
      >
        ↷
      </ToolbarButton>
      <ToolbarDivider />
      <ToolbarButton label="上移块" disabled={!state.editable} onClick={() => moveBlock(-1)}>
        ↑块
      </ToolbarButton>
      <ToolbarButton label="下移块" disabled={!state.editable} onClick={() => moveBlock(1)}>
        ↓块
      </ToolbarButton>
      <ToolbarButton label="复制块" disabled={!state.editable} onClick={copyBlock}>
        ⧉块
      </ToolbarButton>
      <ToolbarButton label="删除块" disabled={!state.editable} onClick={deleteBlock}>
        ×块
      </ToolbarButton>
    </div>
  );
}

interface BubbleMenuBarProps {
  editor: Editor;
}

function BubbleMenuBar({ editor }: BubbleMenuBarProps) {
  // Subscribe to editor state so the bubble buttons reflect the marks
  // applied on the current selection.
  const state = useEditorState({
    editor,
    selector: ({ editor: e }) => ({
      bold: e.isActive("bold"),
      italic: e.isActive("italic"),
      code: e.isActive("code"),
      editable: e.isEditable,
    }),
  });

  return (
    <div
      data-testid="bubble-menu"
      className="flex items-center gap-0.5 rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper)] px-1 py-0.5 shadow-lg"
    >
      <ToolbarButton
        label="加粗"
        active={state.bold}
        disabled={!state.editable}
        onClick={() =>
          editor.chain().focus().extendMarkRange("bold").toggleBold().run()
        }
      >
        <strong>B</strong>
      </ToolbarButton>
      <ToolbarButton
        label="斜体"
        active={state.italic}
        disabled={!state.editable}
        onClick={() =>
          editor.chain().focus().extendMarkRange("italic").toggleItalic().run()
        }
      >
        <em>I</em>
      </ToolbarButton>
      <ToolbarButton
        label="行内代码"
        active={state.code}
        disabled={!state.editable}
        onClick={() =>
          editor.chain().focus().extendMarkRange("code").toggleCode().run()
        }
      >
        <code>{"</>"}</code>
      </ToolbarButton>
      <ToolbarDivider />
      <ToolbarButton
        label="清除格式"
        disabled={!state.editable}
        onClick={() =>
          editor.chain().focus().unsetAllMarks().clearNodes().run()
        }
      >
        ✕
      </ToolbarButton>
    </div>
  );
}

export default function RichEditor({
  initialMarkdown,
  onChange,
  readOnly = false,
}: RichEditorProps) {
  const [slashOpen, setSlashOpen] = useState(false);
  const [slashPos, setSlashPos] = useState(0);
  const editorRef = useRef<HTMLDivElement>(null);

  const editor = useEditor({
    extensions,
    content: initialMarkdown,
    editable: !readOnly,
    editorProps: {
      attributes: {
        class: "prose-agentdocs focus:outline-none min-h-[30rem]",
        role: "textbox",
      },
      handleKeyDown: (view, event) => {
        if (event.key === "/" && !readOnly) {
          // keydown fires before the "/" is inserted, so selection.from is
          // exactly the position where the slash will land.
          setSlashPos(view.state.selection.from);
          setSlashOpen(true);
        }
        if (event.key === "Escape") setSlashOpen(false);
        return false;
      },
    },
    onUpdate: ({ editor }) => {
      onChange(
        (editor.storage as unknown as { markdown: MarkdownStorage }).markdown
          .getMarkdown(),
      );
    },
  });

  // Keep the editable state in sync when the readOnly prop changes.
  // tiptap v3 emits an `update` event by default on setEditable, which
  // would fire onChange right after mount with unchanged content.
  useEffect(() => {
    editor?.setEditable(!readOnly, false);
  }, [editor, readOnly]);

  // Close the slash menu when clicking outside the editor panel.
  useEffect(() => {
    if (!slashOpen) return;
    const onDown = (e: MouseEvent) => {
      if (editorRef.current && !editorRef.current.contains(e.target as Node)) {
        setSlashOpen(false);
      }
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [slashOpen]);

  // Delete the typed "/" first, then run the selected command.
  const runSlash = (run: () => void) => {
    if (!editor) return;
    editor
      .chain()
      .focus()
      .deleteRange({ from: slashPos, to: slashPos + 1 })
      .run();
    run();
    setSlashOpen(false);
  };

  return (
    <div className="hairline-panel relative overflow-hidden" ref={editorRef}>
      {editor && <Toolbar editor={editor} />}
      <EditorContent className="px-4 py-3" editor={editor} />
      {editor && (
        <BubbleMenu
          editor={editor}
          // The panel clips its content (overflow-hidden), so anchor the
          // bubble to the body with fixed positioning to keep it visible
          // when the selection is on the first line.
          appendTo={() => document.body}
          options={{ strategy: "fixed", placement: "top", offset: 8 }}
        >
          <BubbleMenuBar editor={editor} />
        </BubbleMenu>
      )}
      {slashOpen && editor && (
        <div className="absolute left-4 top-12 z-20 w-56 rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper)] p-1 shadow-lg">
          {[
            {
              label: "标题 1",
              hint: "#",
              run: () => editor.chain().focus().toggleHeading({ level: 1 }).run(),
            },
            {
              label: "标题 2",
              hint: "##",
              run: () => editor.chain().focus().toggleHeading({ level: 2 }).run(),
            },
            {
              label: "标题 3",
              hint: "###",
              run: () => editor.chain().focus().toggleHeading({ level: 3 }).run(),
            },
            {
              label: "引用",
              hint: ">",
              run: () => editor.chain().focus().toggleBlockquote().run(),
            },
            {
              label: "代码块",
              hint: "```",
              run: () => editor.chain().focus().toggleCodeBlock().run(),
            },
            {
              label: "无序列表",
              hint: "-",
              run: () => editor.chain().focus().toggleBulletList().run(),
            },
            {
              label: "有序列表",
              hint: "1.",
              run: () => editor.chain().focus().toggleOrderedList().run(),
            },
            {
              label: "插入表格",
              hint: "▦",
              run: () =>
                editor
                  .chain()
                  .focus()
                  .insertTable({ rows: 3, cols: 3, withHeaderRow: true })
                  .run(),
            },
          ].map((item) => (
            <button
              key={item.label}
              type="button"
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => runSlash(item.run)}
              className="flex w-full items-center justify-between rounded-sm px-3 py-1.5 text-left text-sm text-[var(--color-ink-2)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-ink)]"
            >
              <span>{item.label}</span>
              <span className="mono-label text-[var(--color-ink-3)]">{item.hint}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
