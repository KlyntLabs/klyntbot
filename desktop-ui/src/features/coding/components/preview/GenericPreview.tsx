import type { ApprovalPreview } from "./types";

type GenericProps = Extract<ApprovalPreview, { kind: "generic" }>;

export function GenericPreview({ args }: GenericProps) {
  return <pre className="approval-preview__command">{JSON.stringify(args, null, 2)}</pre>;
}
