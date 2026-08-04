import { useEffect, useRef, useState, type ReactNode } from "react";
import type { EditorState } from "@tiptap/pm/state";
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
import { Document } from "@tiptap/extension-document";
import { Paragraph } from "@tiptap/extension-paragraph";
import { Text } from "@tiptap/extension-text";
import { Bold } from "@tiptap/extension-bold";
import { Italic } from "@tiptap/extension-italic";
import { Strike } from "@tiptap/extension-strike";
import { Code } from "@tiptap/extension-code";
import { Heading } from "@tiptap/extension-heading";
import { Blockquote } from "@tiptap/extension-blockquote";
import { CodeBlock } from "@tiptap/extension-code-block";
import { BulletList, ListItem, ListKeymap, OrderedList } from "@tiptap/extension-list";
import { HorizontalRule } from "@tiptap/extension-horizontal-rule";
import { HardBreak } from "@tiptap/extension-hard-break";
import { Dropcursor, Gapcursor, TrailingNode, UndoRedo } from "@tiptap/extensions";
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
// Manually assembled instead of StarterKit: block-level markdown shortcuts
// (#, -, >, ```) are disabled so typing them never instantly converts a
// paragraph into a huge heading/list/quote (Notion-like predictability).
// Inline marks (bold/italic/code) keep their input rules.
const extensions = [
	Document,
	Paragraph,
	Text,
	Bold,
	Italic,
	Strike,
	Code,
	Heading.configure({ levels: [1, 2, 3] }).extend({ addInputRules: () => [] }),
	Blockquote.extend({ addInputRules: () => [] }),
	BulletList.extend({ addInputRules: () => [] }),
	OrderedList.extend({ addInputRules: () => [] }),
	CodeBlock.extend({ addInputRules: () => [] }),
	ListItem,
	ListKeymap,
	HorizontalRule,
	HardBreak,
	Dropcursor,
	Gapcursor,
	TrailingNode,
	UndoRedo,
	Placeholder.configure({ placeholder: "输入 / 唤起命令菜单…" }),
	Link.configure({ openOnClick: false }),
	Image,
	Table.configure({ resizable: true }),
	TableRow,
	TableHeader,
	TableCell,
	Markdown,
];

/**
 * The "/" command menu opens at the start of a paragraph (including an
 * empty one), so typing "/" inside a word, URL, or code block never pops
 * the menu while block-level commands stay reachable from any block.
 */
function isSlashTrigger(state: EditorState): boolean {
	const { $from } = state.selection;
	if ($from.parent.type.name !== "paragraph") return false;
	return $from.parentOffset === 0;
}

/** Commands offered by the "/" menu (module-level so the keydown handler can reference them). */
const SLASH_ITEMS: Array<{
	label: string;
	hint: string;
	run: (editor: Editor) => void;
}> = [
	{
		label: "标题 1",
		hint: "#",
		run: (e) => e.chain().focus().toggleHeading({ level: 1 }).run(),
	},
	{
		label: "标题 2",
		hint: "##",
		run: (e) => e.chain().focus().toggleHeading({ level: 2 }).run(),
	},
	{
		label: "标题 3",
		hint: "###",
		run: (e) => e.chain().focus().toggleHeading({ level: 3 }).run(),
	},
	{
		label: "引用",
		hint: ">",
		run: (e) => e.chain().focus().toggleBlockquote().run(),
	},
	{
		label: "代码块",
		hint: "```",
		run: (e) => e.chain().focus().toggleCodeBlock().run(),
	},
	{
		label: "无序列表",
		hint: "-",
		run: (e) => e.chain().focus().toggleBulletList().run(),
	},
	{
		label: "有序列表",
		hint: "1.",
		run: (e) => e.chain().focus().toggleOrderedList().run(),
	},
	{
		label: "插入表格",
		hint: "▦",
		run: (e) =>
			e
				.chain()
				.focus()
				.insertTable({ rows: 3, cols: 3, withHeaderRow: true })
				.run(),
	},
	{
		label: "插入折叠块",
		hint: "▸",
		run: (e) =>
			e
				.chain()
				.focus()
				.insertContent({
					type: "paragraph",
					content: [{ type: "text", text: ":::details 标题\n\n内容\n:::" }],
				})
				.run(),
	},
	{
		label: "上移块",
		hint: "↑",
		run: (e) => moveBlock(e, -1),
	},
	{
		label: "下移块",
		hint: "↓",
		run: (e) => moveBlock(e, 1),
	},
	{
		label: "复制块",
		hint: "⧉",
		run: (e) => copyBlock(e),
	},
	{
		label: "删除块",
		hint: "×",
		run: (e) => deleteBlock(e),
	},
];

export interface RichEditorProps {
	/** Markdown content used to initialize the editor (parsed by the Markdown extension). */
	initialMarkdown: string;
	/** Called with the serialized markdown whenever the document changes. */
	onChange: (markdown: string) => void;
	/** When true, the editor content is not editable. */
	readOnly?: boolean;
	/** Called when a wiki link is Cmd/Ctrl+clicked inside the editor. */
	onNavigateLink?: (href: string) => void;
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
		<span aria-hidden="true" className="mx-1 h-4 w-px bg-[var(--color-rule)]" />
	);
}

/** Top-level block index under the cursor (used by block move/copy/delete). */
function currentBlockIndex(e: Editor): number {
	const { from } = e.state.selection;
	let idx = 0;
	e.state.doc.forEach((node, offset) => {
		if (offset + node.nodeSize <= from) idx++;
	});
	const total = (e.getJSON().content ?? []).length;
	return Math.min(idx, Math.max(total - 1, 0));
}

function moveBlock(editor: Editor, dir: -1 | 1) {
	const json = editor.getJSON();
	const blocks = [...((json.content ?? []) as object[])];
	const idx = currentBlockIndex(editor);
	const swap = idx + dir;
	if (blocks.length < 2 || swap < 0 || swap >= blocks.length) return;
	[blocks[idx], blocks[swap]] = [blocks[swap], blocks[idx]];
	editor
		.chain()
		.focus()
		.setContent({ ...json, content: blocks })
		.run();
}

function copyBlock(editor: Editor) {
	const json = editor.getJSON();
	const blocks = [...((json.content ?? []) as object[])];
	const idx = currentBlockIndex(editor);
	if (blocks.length === 0) return;
	blocks.splice(idx + 1, 0, JSON.parse(JSON.stringify(blocks[idx])));
	editor
		.chain()
		.focus()
		.setContent({ ...json, content: blocks })
		.run();
}

function deleteBlock(editor: Editor) {
	const json = editor.getJSON();
	const blocks = [...((json.content ?? []) as object[])];
	const idx = currentBlockIndex(editor);
	if (blocks.length === 0) return;
	blocks.splice(idx, 1);
	if (blocks.length === 0) {
		blocks.push({ type: "paragraph" });
	}
	editor
		.chain()
		.focus()
		.setContent({ ...json, content: blocks })
		.run();
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
	onNavigateLink,
}: RichEditorProps) {
	const [slashOpen, setSlashOpen] = useState(false);
	const [slashPos, setSlashPos] = useState(0);
	const [slashQuery, setSlashQuery] = useState("");
	// Refs mirror the slash-menu state so the keydown/onUpdate handlers (whose
	// closures are captured once when the editor is created) read fresh values.
	const slashOpenRef = useRef(false);
	const slashPosRef = useRef(0);
	const slashQueryRef = useRef("");
	const editorRef = useRef<HTMLDivElement>(null);

	const openSlash = (pos: number) => {
		slashPosRef.current = pos;
		slashOpenRef.current = true;
		slashQueryRef.current = "";
		setSlashPos(pos);
		setSlashQuery("");
		setSlashOpen(true);
	};
	const closeSlash = () => {
		slashOpenRef.current = false;
		setSlashOpen(false);
	};

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
				if (event.key === "/" && !readOnly && isSlashTrigger(view.state)) {
					// keydown fires before the "/" is inserted, so selection.from is
					// exactly the position where the slash will land.
					openSlash(view.state.selection.from);
					return false;
				}
				if (event.key === "Escape") {
					closeSlash();
					return false;
				}
				if (slashOpenRef.current && event.key === "Backspace") {
					// Backspacing over the "/" itself closes the menu.
					if (view.state.selection.from <= slashPosRef.current + 1) {
						closeSlash();
					}
					return false;
				}
				return false;
			},
			handlePaste: (view, event) => {
				const items = event.clipboardData?.items;
				if (!items) return false;
				for (const item of items) {
					if (item.type.startsWith("image/")) {
						const file = item.getAsFile();
						if (!file) continue;
						if (file.size > 2 * 1024 * 1024) {
							// eslint-disable-next-line no-alert
							window.alert("图片超过 2 MiB，无法内联粘贴");
							return true;
						}
						const reader = new FileReader();
						reader.onload = () => {
							const src = reader.result as string;
							view.dispatch(
								view.state.tr.replaceSelectionWith(
									view.state.schema.nodes.image.create({ src }),
								),
							);
						};
						reader.readAsDataURL(file);
						return true;
					}
				}
				return false;
			},
			// Cmd/Ctrl+click on a wiki link navigates instead of placing the caret.
			handleClick: (_view, _pos, event) => {
				if (!event.metaKey && !event.ctrlKey) return false;
				const anchor = (event.target as HTMLElement | null)?.closest?.("a");
				const href = anchor?.getAttribute("href") ?? "";
				if (!href) return false;
				if (href.startsWith("/projects/")) {
					onNavigateLink?.(href);
					return true;
				}
				return false;
			},
		},
		onUpdate: ({ editor }) => {
			onChange(
				(
					editor.storage as unknown as { markdown: MarkdownStorage }
				).markdown.getMarkdown(),
			);
			// Keep the "/" query in sync with what was typed after the slash.
			// onUpdate runs after each transaction, so the doc text is fresh
			// (unlike the keydown handler, which runs before insertion).
			if (slashOpenRef.current) {
				const from = editor.state.selection.from;
				const text =
					from > slashPosRef.current
						? editor.state.doc.textBetween(slashPosRef.current, from)
						: "";
				slashQueryRef.current = text.startsWith("/") ? text.slice(1) : "";
				setSlashQuery(slashQueryRef.current);
			}
		},
	});

	// Keep the editable state in sync when the readOnly prop changes.
	// tiptap v3 emits an `update` event by default on setEditable, which
	// would fire onChange right after mount with unchanged content.
	useEffect(() => {
		editor?.setEditable(!readOnly, false);
	}, [editor, readOnly]);

	// Sync when initialMarkdown arrives after mount (async raw load). Only
	// fill when the editor is still empty so in-progress typing is preserved.
	const editorRefReady = useRef(false);
	useEffect(() => {
		if (!editor || !initialMarkdown) return;
		if (!editorRefReady.current) {
			editorRefReady.current = true;
			editor.commands.setContent(initialMarkdown, { emitUpdate: false }); // no update event
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [editor, initialMarkdown]);

	// Close the slash menu when clicking outside the editor panel.
	useEffect(() => {
		if (!slashOpen) return;
		const onDown = (e: MouseEvent) => {
			if (editorRef.current && !editorRef.current.contains(e.target as Node)) {
				closeSlash();
			}
		};
		document.addEventListener("mousedown", onDown);
		return () => document.removeEventListener("mousedown", onDown);
	}, [slashOpen]);

	// Delete the typed "/" first (plus any query text typed after it), then
	// run the selected command. The deleted range is validated against the
	// actual document text so an unrelated caret move never deletes a chunk.
	const runSlash = (run: (editor: Editor) => void) => {
		if (!editor) return;
		const { from } = editor.state.selection;
		const typed =
			from > slashPos ? editor.state.doc.textBetween(slashPos, from) : "";
		const to = typed.startsWith("/") ? from : slashPos + 1;
		editor.chain().focus().deleteRange({ from: slashPos, to }).run();
		run(editor);
		closeSlash();
	};

	// The query is whatever was typed after "/", kept fresh by onUpdate.
	const q = slashQuery.trim().toLowerCase();
	const slashItems = SLASH_ITEMS.filter(
		(item) =>
			item.label.toLowerCase().includes(q) || item.hint.toLowerCase().includes(q),
	);

	return (
		<div
			className="relative overflow-hidden"
			data-editor-panel
			ref={editorRef}
		>
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
				<div className="absolute left-4 top-12 z-20 max-h-80 w-44 overflow-y-auto rounded-[var(--radius)] border border-[var(--color-rule)] bg-[var(--color-paper)] p-0.5 shadow-lg">
					{slashItems.length === 0 ? (
						<p className="px-3 py-1.5 text-sm text-[var(--color-ink-3)]">
							无匹配项
						</p>
					) : (
						slashItems.map((item) => (
							<button
								key={item.label}
								type="button"
								onMouseDown={(e) => e.preventDefault()}
								onClick={() => runSlash(item.run)}
								className="flex w-full items-center justify-between gap-2 rounded-sm px-2 py-1 text-left text-[13px] leading-tight text-[var(--color-ink-2)] hover:bg-[var(--color-surface-accent)] hover:text-[var(--color-ink)]"
							>
								<span>{item.label}</span>
								<span className="mono-label text-[var(--color-ink-3)]">
									{item.hint}
								</span>
							</button>
						))
					)}
				</div>
			)}
		</div>
	);
}
