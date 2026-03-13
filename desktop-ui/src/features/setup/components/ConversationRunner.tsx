import { useCallback, useRef } from "react";
import { useNavigate } from "react-router";
import { useConversationRunner } from "../hooks/useConversationRunner";
import type { ConversationNode, NodeValue } from "../schema";
import { FinancePanel } from "./FinancePanel";
import { InlineInput } from "./InlineInput";
import { InlineMasked } from "./InlineMasked";
import { InlineSelect } from "./InlineSelect";
import { InlineTags } from "./InlineTags";
import { TypewriterText } from "./TypewriterText";

// ── Completed node (locked text, click to edit) ──────────────

function CompletedNode({
  node,
  value,
  isEditing,
  onClickEdit,
  onResubmit,
}: {
  node: ConversationNode;
  value: NodeValue;
  isEditing: boolean;
  onClickEdit: () => void;
  onResubmit: (value: NodeValue) => void;
}) {
  const parts = node.prompt.split("{input}");
  const before = parts[0] ?? "";
  const after = parts[1] ?? "";

  if (isEditing) {
    return (
      <div className="text-foreground text-base leading-relaxed">
        <span>{before}</span>
        {renderInput(node, value, onResubmit, true)}
        <span>{after}</span>
      </div>
    );
  }

  // Display completed value
  const displayValue = formatValue(node, value);
  return (
    <div
      className="text-foreground text-base leading-relaxed cursor-pointer group"
      onClick={onClickEdit}
      onKeyDown={(e) => e.key === "Enter" && onClickEdit()}
      role="button"
      tabIndex={0}
    >
      <span>{before}</span>
      <span className="font-semibold group-hover:text-accent transition-colors">
        {displayValue}
      </span>
      <span>{after}</span>
    </div>
  );
}

function formatValue(node: ConversationNode, value: NodeValue): string {
  if (node.inputType === "select" || node.inputType === "confirm") {
    if (node.inputType === "confirm") return value ? "Yes" : "No";
    const opt = node.options?.find((o) => o.value === value);
    return opt?.label ?? String(value);
  }
  if (node.inputType === "masked" && typeof value === "string") {
    return value.length > 8 ? `${"•".repeat(8)}${value.slice(-4)}` : "•".repeat(value.length);
  }
  if (Array.isArray(value)) return value.join(", ");
  return String(value);
}

// ── Render input by type ─────────────────────────────────────

function renderInput(
  node: ConversationNode,
  defaultValue: NodeValue | undefined,
  onSubmit: (value: NodeValue) => void,
  autoFocus = true,
) {
  switch (node.inputType) {
    case "text":
      return (
        <InlineInput
          defaultValue={typeof defaultValue === "string" ? defaultValue : ""}
          onSubmit={onSubmit}
          autoFocus={autoFocus}
        />
      );
    case "select":
      return (
        <InlineSelect
          options={node.options ?? []}
          defaultValue={typeof defaultValue === "string" ? defaultValue : (node.default as string)}
          onSubmit={onSubmit}
          autoFocus={autoFocus}
        />
      );
    case "masked":
      return (
        <InlineMasked
          defaultValue={typeof defaultValue === "string" ? defaultValue : ""}
          onSubmit={onSubmit}
          autoFocus={autoFocus}
        />
      );
    case "confirm":
      return (
        <InlineSelect
          options={[
            { label: "Yes", value: "true" },
            { label: "No", value: "false" },
          ]}
          defaultValue={
            defaultValue === true || defaultValue === "true" || node.default === true
              ? "true"
              : "false"
          }
          onSubmit={(v) => onSubmit(v === "true")}
          autoFocus={autoFocus}
        />
      );
    case "tags":
      return (
        <InlineTags
          defaultValue={
            Array.isArray(defaultValue) ? defaultValue : ((node.default as string[]) ?? [])
          }
          onSubmit={onSubmit}
          autoFocus={autoFocus}
        />
      );
    default:
      return null;
  }
}

// ── Main ConversationRunner ──────────────────────────────────

export function ConversationRunner() {
  const navigate = useNavigate();
  const containerRef = useRef<HTMLDivElement>(null);
  const {
    transcript,
    activeNode,
    activeIndex,
    isAnimating,
    error,
    isSaving,
    isLoading,
    progress,
    schema,
    submit,
    resubmit,
    startEdit,
    setAnimationComplete,
    completeFinancePanel,
  } = useConversationRunner();

  const isAnyEditing = Object.values(transcript).some((e) => e.status === "editing");

  const handleSubmit = useCallback(
    async (value: NodeValue) => {
      await submit(value);
      // Scroll to bottom after new node appears
      requestAnimationFrame(() => {
        containerRef.current?.scrollTo({
          top: containerRef.current.scrollHeight,
          behavior: "smooth",
        });
      });
    },
    [submit],
  );

  const handleComplete = useCallback(async () => {
    // Must call config_mark_setup_completed before navigating,
    // otherwise app_info().setupCompleted is still false → redirect loop
    const completeNode = schema.find((n) => n.id === "complete");
    if (completeNode?.save) await completeNode.save(true, {});
    navigate("/");
  }, [navigate, schema]);

  if (isLoading) {
    return (
      <div className="fixed inset-0 flex items-center justify-center bg-surface-base">
        <div className="text-muted text-sm">Loading...</div>
      </div>
    );
  }

  const isComplete = activeNode?.id === "complete" || activeIndex >= schema.length;

  return (
    <div className="fixed inset-0 flex flex-col bg-surface-base">
      {/* Progress bar */}
      <div className="h-1 bg-border">
        <div
          className="h-full bg-accent transition-all duration-500 ease-out"
          style={{ width: `${progress * 100}%` }}
        />
      </div>

      {/* Conversation area */}
      <div ref={containerRef} className="flex-1 overflow-y-auto flex justify-center">
        <div className="w-full max-w-[640px] px-6 py-12 space-y-4">
          {/* Completed & editing nodes */}
          {schema.slice(0, activeIndex).map((node) => {
            const entry = transcript[node.id];
            if (!entry) return null; // Skipped by condition
            if (node.inputType === "complex") return null; // Finance panel handled separately

            return (
              <div
                key={node.id}
                className={`transition-opacity duration-200 ${
                  isAnyEditing && entry.status !== "editing" ? "opacity-40" : ""
                }`}
              >
                <CompletedNode
                  node={node}
                  value={entry.value}
                  isEditing={entry.status === "editing"}
                  onClickEdit={() => !isAnyEditing && startEdit(node.id)}
                  onResubmit={(v) => resubmit(node.id, v)}
                />
              </div>
            );
          })}

          {/* Active node */}
          {activeNode && !isComplete && activeNode.inputType !== "complex" && (
            <div className="text-foreground text-base leading-relaxed">
              <TypewriterText
                text={activeNode.prompt}
                onComplete={setAnimationComplete}
                showInput={!isAnimating}
                input={renderInput(activeNode, activeNode.default, handleSubmit)}
              />
            </div>
          )}

          {/* Finance panel (complex node) */}
          {activeNode?.id === "finance_setup" && <FinancePanel onComplete={completeFinancePanel} />}

          {/* Error message */}
          {error && (
            <p className="text-[12px] text-destructive animate-in fade-in duration-200">{error}</p>
          )}

          {/* Saving indicator */}
          {isSaving && (
            <p className="text-[12px] text-muted animate-in fade-in duration-200">Saving...</p>
          )}

          {/* Complete state */}
          {isComplete && (
            <div className="space-y-6">
              <TypewriterText
                text="Great, we're all set. Let's get started!"
                onComplete={() => {}}
              />
              <button
                type="button"
                onClick={handleComplete}
                className="px-6 py-2.5 text-[14px] font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-colors"
              >
                Launch Klynt
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
