import { NodeViewContent, type NodeViewProps, NodeViewWrapper } from "@tiptap/react";
import { Check, Copy, X } from "lucide-react";
import { useState } from "react";

const LANGUAGE_LABELS: Record<string, string> = {
  js: "JavaScript",
  javascript: "JavaScript",
  ts: "TypeScript",
  typescript: "TypeScript",
  jsx: "JSX",
  tsx: "TSX",
  py: "Python",
  python: "Python",
  rb: "Ruby",
  ruby: "Ruby",
  rs: "Rust",
  rust: "Rust",
  go: "Go",
  java: "Java",
  cpp: "C++",
  "c++": "C++",
  c: "C",
  cs: "C#",
  csharp: "C#",
  swift: "Swift",
  kt: "Kotlin",
  kotlin: "Kotlin",
  sql: "SQL",
  html: "HTML",
  css: "CSS",
  scss: "SCSS",
  json: "JSON",
  yaml: "YAML",
  yml: "YAML",
  toml: "TOML",
  xml: "XML",
  md: "Markdown",
  markdown: "Markdown",
  bash: "Bash",
  sh: "Shell",
  shell: "Shell",
  zsh: "Zsh",
  fish: "Fish",
  powershell: "PowerShell",
  dockerfile: "Dockerfile",
  graphql: "GraphQL",
  lua: "Lua",
  r: "R",
  dart: "Dart",
  elixir: "Elixir",
  erlang: "Erlang",
  haskell: "Haskell",
  scala: "Scala",
  php: "PHP",
  perl: "Perl",
  text: "Plain Text",
};

function getLanguageLabel(lang: string | null | undefined): string {
  if (!lang || lang === "null") return "Code";
  return LANGUAGE_LABELS[lang.toLowerCase()] ?? lang;
}

export function CodeBlockView({ node, updateAttributes, deleteNode, editor }: NodeViewProps) {
  const [copied, setCopied] = useState(false);
  const language = node.attrs.language as string | null;

  const handleCopy = () => {
    const text = node.textContent;
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleDelete = () => {
    deleteNode();
  };

  const handleLanguageChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    updateAttributes({ language: e.target.value || null });
  };

  const isEditable = editor?.isEditable ?? true;

  return (
    <NodeViewWrapper className="code-block-wrapper">
      <div className="code-block-header" contentEditable={false}>
        <select
          className="code-block-lang-select"
          value={language ?? ""}
          onChange={handleLanguageChange}
          disabled={!isEditable}
        >
          <option value="">Auto</option>
          {Object.entries(LANGUAGE_LABELS)
            .filter(([key]) => key === key.toLowerCase() && key.length > 2)
            .sort(([, a], [, b]) => a.localeCompare(b))
            .map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
        </select>
        <span className="code-block-lang-label">{getLanguageLabel(language)}</span>
        <div className="code-block-actions">
          {isEditable && (
            <button type="button" className="code-block-btn" onClick={handleDelete} title="Remove">
              <X className="size-3.5" />
            </button>
          )}
          <button type="button" className="code-block-btn" onClick={handleCopy} title="Copy code">
            {copied ? (
              <>
                <Check className="size-3.5" />
                <span>Copied</span>
              </>
            ) : (
              <>
                <Copy className="size-3.5" />
                <span>Copy</span>
              </>
            )}
          </button>
        </div>
      </div>
      <pre>
        <NodeViewContent as="code" />
      </pre>
    </NodeViewWrapper>
  );
}
