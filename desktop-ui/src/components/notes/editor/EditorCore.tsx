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
import { SlashCommandsExtension } from "./SlashCommandMenu";

const lowlight = createLowlight(common);

export function getEditorExtensions(extra: Extensions = []): Extensions {
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
    SlashCommandsExtension,
    ...extra,
  ];
}

interface UseNoteEditorOptions {
  content: string;
  onUpdate: (html: string, text: string) => void;
  extensions?: Extensions;
}

export function useNoteEditor({ content, onUpdate, extensions = [] }: UseNoteEditorOptions) {
  return useEditor({
    extensions: getEditorExtensions(extensions),
    content,
    onUpdate: ({ editor: ed }) => {
      onUpdate(ed.getHTML(), ed.getText());
    },
    editorProps: {
      attributes: { class: "editor-content" },
    },
  });
}

interface EditorContentWrapperProps {
  editor: ReturnType<typeof useEditor>;
}

export function EditorContentWrapper({ editor }: EditorContentWrapperProps) {
  return <EditorContent editor={editor} className="flex-1 min-h-0 overflow-y-auto" />;
}
