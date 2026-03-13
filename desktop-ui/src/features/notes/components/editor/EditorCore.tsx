import { ipc } from "@shared/hooks/useIpc";
import { isTauri } from "@shared/lib/utils";
import { convertFileSrc } from "@tauri-apps/api/core";
import CodeBlockLowlight from "@tiptap/extension-code-block-lowlight";
import Color from "@tiptap/extension-color";
import Highlight from "@tiptap/extension-highlight";
import Image from "@tiptap/extension-image";
import Link from "@tiptap/extension-link";
import Placeholder from "@tiptap/extension-placeholder";
import Subscript from "@tiptap/extension-subscript";
import Superscript from "@tiptap/extension-superscript";
import { Table, TableCell, TableHeader, TableRow } from "@tiptap/extension-table";
import TaskItem from "@tiptap/extension-task-item";
import TaskList from "@tiptap/extension-task-list";
import TextAlign from "@tiptap/extension-text-align";
import { TextStyle } from "@tiptap/extension-text-style";
import Typography from "@tiptap/extension-typography";
import Underline from "@tiptap/extension-underline";
import type { Extensions } from "@tiptap/react";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { common, createLowlight } from "lowlight";
import { useEffect } from "react";
import { Markdown } from "tiptap-markdown";
import { EntityMentionAutocomplete, EntityMentionMark } from "./EntityMention";
import { MathBlock, MathInline } from "./MathNode";
import { SlashCommandsExtension } from "./SlashCommandMenu";
import { VimModeExtension, type VimModeOptions } from "./vim";
import { WikiLinkAutocomplete, WikiLinkMark } from "./WikiLinkNode";

// ── Entity Resolution Hook ──────────────────────────────────────────────
// Checks whether entity mentions (@task, @project) and wiki links ([[note]])
// reference entities that actually exist. Toggles CSS classes defined in
// editor.css (.wiki-link--unresolved, .entity-mention--unresolved) to gray out
// unresolved references.

export function useEntityResolution(editor: ReturnType<typeof useEditor>) {
  useEffect(() => {
    if (!editor) return;

    const cache = new Map<string, boolean>();
    let timer: ReturnType<typeof setTimeout> | null = null;

    async function resolve() {
      const root = editor!.view.dom;
      const mentions = root.querySelectorAll<HTMLElement>("span[data-entity-mention]");
      const wikiLinks = root.querySelectorAll<HTMLElement>("span[data-wiki-link]");

      if (mentions.length === 0 && wikiLinks.length === 0) return;

      const taskIds = new Set<string>();
      const projectIds = new Set<string>();
      const noteIds = new Set<string>();

      for (const el of mentions) {
        const type = el.dataset.entityType || "";
        const id = el.dataset.entityId || "";
        if (!id) continue;
        if (!cache.has(`${type}:${id}`)) {
          if (type === "task") taskIds.add(id);
          else if (type === "project") projectIds.add(id);
        }
      }
      for (const el of wikiLinks) {
        const id = el.dataset.noteId || "";
        if (id && !cache.has(`note:${id}`)) noteIds.add(id);
      }

      if (taskIds.size > 0 || projectIds.size > 0 || noteIds.size > 0) {
        try {
          const promises: Promise<void>[] = [];
          if (taskIds.size > 0) {
            promises.push(
              ipc<{ id: string }[]>("task_list", undefined).then((tasks) => {
                const idSet = new Set(tasks.map((t) => t.id));
                for (const id of taskIds) cache.set(`task:${id}`, idSet.has(id));
              }),
            );
          }
          if (projectIds.size > 0) {
            promises.push(
              ipc<{ id: string }[]>("project_list", undefined).then((projects) => {
                const idSet = new Set(projects.map((p) => p.id));
                for (const id of projectIds) cache.set(`project:${id}`, idSet.has(id));
              }),
            );
          }
          if (noteIds.size > 0) {
            promises.push(
              ipc<{ id: string }[]>("note_list", undefined).then((notes) => {
                const idSet = new Set(notes.map((n) => n.id));
                for (const id of noteIds) cache.set(`note:${id}`, idSet.has(id));
              }),
            );
          }
          await Promise.all(promises);
        } catch (e) {
          console.warn("Entity resolution failed:", e);
          return;
        }
      }

      // Toggle CSS classes for unresolved entities (classes defined in editor.css)
      for (const el of wikiLinks) {
        const id = el.dataset.noteId || "";
        if (id) el.classList.toggle("wiki-link--unresolved", cache.get(`note:${id}`) === false);
      }
      for (const el of mentions) {
        const type = el.dataset.entityType || "";
        const id = el.dataset.entityId || "";
        if (id)
          el.classList.toggle("entity-mention--unresolved", cache.get(`${type}:${id}`) === false);
      }
    }

    // Initial check
    timer = setTimeout(resolve, 300);

    // Re-check on editor updates
    const onUpdate = () => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(resolve, 500);
    };
    editor.on("update", onUpdate);

    return () => {
      if (timer) clearTimeout(timer);
      editor.off("update", onUpdate);
    };
  }, [editor]);
}

const lowlight = createLowlight(common);
const NOOP = () => {};
const EMPTY_EXTENSIONS: Extensions = [];

interface EditorExtensionOptions {
  extra?: Extensions;
  onNavigateNote?: (noteId: string) => void;
  onNavigateEntity?: (entityType: string, entityId: string) => void;
  vimOptions?: VimModeOptions;
}

export function getEditorExtensions(opts: EditorExtensionOptions = {}): Extensions {
  const { extra = [], onNavigateNote, onNavigateEntity, vimOptions } = opts;
  return [
    StarterKit.configure({ codeBlock: false }),
    Placeholder.configure({
      includeChildren: true,
      placeholder: ({ node }) => {
        if (node.type.name === "heading") return "Heading";
        return "Write something, or type / for commands...";
      },
    }),
    TaskList,
    TaskItem.configure({ nested: true }),
    Table.configure({ resizable: true }),
    TableRow,
    TableCell,
    TableHeader,
    Link.configure({
      openOnClick: false,
      HTMLAttributes: { class: "editor-link" },
    }),
    Image.configure({
      inline: true,
      HTMLAttributes: { class: "editor-image" },
    }),
    Highlight.configure({ multicolor: true }),
    Typography,
    Underline,
    Subscript,
    Superscript,
    TextStyle,
    Color,
    CodeBlockLowlight.configure({
      lowlight,
      defaultLanguage: "text",
    }),
    TextAlign.configure({ types: ["heading", "paragraph"] }),
    WikiLinkMark.configure({ onNavigate: onNavigateNote ?? NOOP }),
    WikiLinkAutocomplete,
    EntityMentionMark.configure({ onNavigate: onNavigateEntity ?? NOOP }),
    EntityMentionAutocomplete,
    MathBlock,
    MathInline,
    SlashCommandsExtension,
    VimModeExtension.configure(vimOptions ?? {}),
    Markdown.configure({
      html: true,
      transformPastedText: true,
      transformCopiedText: true,
    }),
    ...extra,
  ];
}

interface UseNoteEditorOptions {
  content: string;
  onUpdate: (html: string, markdown: string) => void;
  extensions?: Extensions;
  onNavigateNote?: (noteId: string) => void;
  onNavigateEntity?: (entityType: string, entityId: string) => void;
  vimOptions?: VimModeOptions;
}

/** Convert a file to base64 string. */
function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result as string;
      // Strip the data:...;base64, prefix
      resolve(result.split(",")[1]);
    };
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

export function useNoteEditor({
  content,
  onUpdate,
  extensions = EMPTY_EXTENSIONS,
  onNavigateNote,
  onNavigateEntity,
  vimOptions,
}: UseNoteEditorOptions) {
  return useEditor({
    extensions: getEditorExtensions({
      extra: extensions,
      onNavigateNote,
      onNavigateEntity,
      vimOptions,
    }),
    content,
    onUpdate: ({ editor: ed }) => {
      onUpdate(ed.getHTML(), ed.storage.markdown.getMarkdown());
    },
    editorProps: {
      attributes: { class: "editor-content" },
      scrollThreshold: { top: 80, bottom: 80, left: 0, right: 0 },
      scrollMargin: { top: 80, bottom: 80, left: 0, right: 0 },
      handlePaste: (view, event) => {
        const items = event.clipboardData?.files;
        if (!items || items.length === 0) return false;

        const imageFile = Array.from(items).find((f) => f.type.startsWith("image/"));
        if (!imageFile) return false;

        event.preventDefault();

        // Save async — insert image once backend returns the URL
        (async () => {
          try {
            const base64 = await fileToBase64(imageFile);
            const ext = imageFile.type.split("/")[1] || "png";
            const savedPath = await ipc<string>("note_save_attachment", {
              data: base64,
              filename: `paste.${ext}`,
            });

            // Tauri returns absolute path → convert to asset URL
            // Dev-api returns relative URL like /attachments/uuid.png
            const src = isTauri ? convertFileSrc(savedPath) : savedPath;

            const { tr } = view.state;
            const imageNode = view.state.schema.nodes.image.create({ src });
            view.dispatch(tr.replaceSelectionWith(imageNode));
          } catch (e) {
            console.error("Failed to paste image:", e);
          }
        })();

        return true;
      },
    },
  });
}

interface EditorContentWrapperProps {
  editor: ReturnType<typeof useEditor>;
  className?: string;
}

export function EditorContentWrapper({ editor, className }: EditorContentWrapperProps) {
  return (
    <EditorContent editor={editor} className={className ?? "flex-1 min-h-0 overflow-y-auto"} />
  );
}
