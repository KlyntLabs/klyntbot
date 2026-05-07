import type { ApprovalPreview } from "./types";
import { CommandPreview } from "./CommandPreview";
import { DiffPreview } from "./DiffPreview";
import { GenericPreview } from "./GenericPreview";
import { McpPreview } from "./McpPreview";
import { UrlPreview } from "./UrlPreview";

export function PreviewRenderer({ preview }: { preview: ApprovalPreview }) {
  switch (preview.kind) {
    case "diff":
      return <DiffPreview {...preview} />;
    case "command":
      return <CommandPreview {...preview} />;
    case "url":
      return <UrlPreview {...preview} />;
    case "mcp":
      return <McpPreview {...preview} />;
    case "generic":
      return <GenericPreview {...preview} />;
    default: {
      const _exhaustive: never = preview;
      return <pre>{JSON.stringify(_exhaustive, null, 2)}</pre>;
    }
  }
}
