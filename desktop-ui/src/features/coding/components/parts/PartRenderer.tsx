import { CommandExecutionPart } from "./CommandExecutionPart";
import { FileChangePart } from "./FileChangePart";
import { FinishPart } from "./FinishPart";
import { ReasoningPart } from "./ReasoningPart";
import { TextPart } from "./TextPart";
import { ToolCallPart } from "./ToolCallPart";
import { ToolResultPart } from "./ToolResultPart";
import type { MessagePart } from "./types";

export function PartRenderer({ part }: { part: MessagePart }) {
  switch (part.kind) {
    case "text":
      return <TextPart text={part.text} />;
    case "tool_call":
      return <ToolCallPart callId={part.call_id} name={part.name} args={part.args} />;
    case "tool_result":
      return <ToolResultPart callId={part.call_id} output={part.output} isError={part.is_error} />;
    case "reasoning":
      return <ReasoningPart text={part.text} redacted={part.redacted} />;
    case "file_change":
      return (
        <FileChangePart path={part.path} diffUnified={part.diff_unified} applied={part.applied} />
      );
    case "command_execution":
      return (
        <CommandExecutionPart
          command={part.command}
          exitCode={part.exit_code}
          stdout={part.stdout}
          stderr={part.stderr}
        />
      );
    case "finish":
      return <FinishPart reason={part.reason} />;
    default:
      return null;
  }
}
