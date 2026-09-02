import { ThinkingDots } from "@shared/ui/ThinkingDots";
import { useCallback, useRef } from "react";
import { useNavigate } from "react-router";
import { useConversationRunner } from "../hooks/useConversationRunner";
import type { ConversationNode, NodeValue } from "../schema";
import { InlineCheckboxList } from "./InlineCheckboxList";
import { InlineInput } from "./InlineInput";
import { InlineMasked } from "./InlineMasked";
import { InlineSelect } from "./InlineSelect";
import { InlineTags } from "./InlineTags";
import { TypewriterText } from "./TypewriterText";

// ── Step indicator dots ─────────────────────────────────────

function StepDots({
  total,
  current,
  completed,
}: {
  total: number;
  current: number;
  completed: number;
}) {
  return (
    <div className="flex items-center gap-2">
      {Array.from({ length: total }, (_, i) => {
        const isDone = i < completed;
        const isActive = i === current;
        return (
          <div
            // biome-ignore lint/suspicious/noArrayIndexKey: static step indicator dots from Array.from
            key={`step-${i}`}
            className={`rounded-full transition-all duration-300 ${
              isActive ? "w-6 h-2 bg-brand" : isDone ? "size-2 bg-brand/60" : "size-2 bg-control-hover"
            }`}
          />
        );
      })}
    </div>
  );
}

// ── Completed node (compact summary, click to edit) ─────────

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
      <div className="py-3 px-4 rounded-xl bg-control-hover border border-separator">
        <div className="text-lg text-fg leading-relaxed">
          <span className="text-fg-secondary">{before}</span>
          {renderInput(node, value, onResubmit, true)}
          <span className="text-fg-secondary">{after}</span>
        </div>
      </div>
    );
  }

  const displayValue = formatValue(node, value);
  return (
    <button
      type="button"
      onClick={onClickEdit}
      className="w-full text-left py-2 px-4 rounded-xl hover:bg-control-hover transition-colors group"
    >
      <span className="text-[15px] text-fg-secondary">{before}</span>
      <span className="text-[15px] font-semibold text-fg group-hover:text-brand transition-colors">
        {displayValue}
      </span>
      <span className="text-[15px] text-fg-secondary">{after}</span>
    </button>
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
  if (node.inputType === "checkbox-list" && Array.isArray(value)) {
    if (value.length === 0) return "None";
    return value
      .map((v) => node.checkboxOptions?.find((o) => o.value === v)?.label ?? v)
      .join(", ");
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
    case "checkbox-list":
      return (
        <InlineCheckboxList
          options={node.checkboxOptions ?? []}
          defaultValue={Array.isArray(defaultValue) ? defaultValue : []}
          onSubmit={(values) => onSubmit(values)}
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
    schema,
    submit,
    resubmit,
    startEdit,
    setAnimationComplete,
  } = useConversationRunner();

  const isAnyEditing = Object.values(transcript).some((e) => e.status === "editing");

  const visibleNodes = schema.filter((n) => n.id !== "complete");
  const completedCount = Object.values(transcript).filter((e) => e.status === "completed").length;

  const handleSubmit = useCallback(
    async (value: NodeValue) => {
      await submit(value);
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
    const completeNode = schema.find((n) => n.id === "complete");
    if (completeNode?.save) await completeNode.save(true, {});
    navigate("/");
  }, [navigate, schema]);

  if (isLoading) {
    return (
      <div className="fixed inset-0 flex items-center justify-center">
        <ThinkingDots />
      </div>
    );
  }

  const isComplete = activeNode?.id === "complete" || activeIndex >= schema.length;

  return (
    <div className="fixed inset-0 flex flex-col items-center overflow-y-auto py-12">
      {/* Central card */}
      <div className="relative w-full max-w-[580px] mx-auto px-4 my-auto">
        {/* Logo + title */}
        <div className="text-center mb-8">
          <div className="w-14 h-14 mx-auto mb-4 rounded-2xl bg-brand/10 border border-brand/20 flex items-center justify-center">
            <span className="text-2xl font-bold text-brand">K</span>
          </div>
          {!isComplete && (
            <h1 className="text-2xl font-semibold text-fg tracking-tight">
              {activeIndex === 0 && !Object.keys(transcript).length
                ? "Welcome to Klynt"
                : "Setting up"}
            </h1>
          )}
        </div>

        {/* Glass card container */}
        <div
          ref={containerRef}
          className="island rounded-2xl border border-separator p-8"
          style={{ animation: "glass-appear 0.3s ease-out" }}
        >
          {/* Completed nodes (compact list) */}
          {schema.slice(0, activeIndex).map((node) => {
            const entry = transcript[node.id];
            if (!entry) return null;
            if (node.inputType === "complex") return null;

            return (
              <div
                key={node.id}
                className={`transition-opacity duration-200 ${
                  isAnyEditing && entry.status !== "editing" ? "opacity-30" : ""
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

          {/* Divider between completed and active */}
          {activeIndex > 0 && !isComplete && activeNode?.inputType !== "complex" && (
            <div className="my-4 border-t border-separator" />
          )}

          {/* Active node — larger, prominent */}
          {activeNode && !isComplete && activeNode.inputType !== "complex" && (
            <div className="py-2" style={{ animation: "fade-in-up 0.3s ease-out" }}>
              <div className="text-xl text-fg leading-relaxed">
                <TypewriterText
                  text={activeNode.prompt}
                  onComplete={setAnimationComplete}
                  showInput={!isAnimating}
                  input={renderInput(activeNode, activeNode.default, handleSubmit)}
                />
              </div>
            </div>
          )}

          {/* Error */}
          {error && (
            <div className="mt-3 px-3 py-2 rounded-lg bg-status-danger/10 border border-status-danger/20">
              <p className="text-sm text-status-danger">{error}</p>
            </div>
          )}

          {/* Saving */}
          {isSaving && (
            <div className="mt-3 flex items-center gap-2 text-fg-secondary">
              <ThinkingDots size="sm" />
              <span className="text-sm">Saving...</span>
            </div>
          )}

          {/* Complete state */}
          {isComplete && (
            <div className="text-center py-6" style={{ animation: "fade-in-up 0.4s ease-out" }}>
              <div className="size-16 mx-auto mb-5 rounded-full bg-status-success/10 border border-status-success/20 flex items-center justify-center">
                <svg
                  className="size-8 text-status-success"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={2}
                  role="img"
                  aria-label="Setup complete"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                </svg>
              </div>
              <h2 className="text-2xl font-semibold text-fg mb-2">You're all set!</h2>
              <p className="text-fg-secondary text-base mb-6">
                Klynt is ready to help you stay productive.
              </p>
              <button
                type="button"
                onClick={handleComplete}
                className="px-8 py-3 text-base font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-all duration-200 hover:scale-[1.02] active:scale-[0.98]"
              >
                Launch Klynt
              </button>
            </div>
          )}
        </div>

        {/* Step dots — below card */}
        {!isComplete && (
          <div className="flex justify-center mt-6">
            <StepDots
              total={visibleNodes.length}
              current={activeIndex}
              completed={completedCount}
            />
          </div>
        )}

        {/* Progress percentage */}
        {!isComplete && (
          <p className="text-center text-ui-sm text-fg-dim mt-3">
            Step {Math.min(activeIndex + 1, visibleNodes.length)} of {visibleNodes.length}
          </p>
        )}
      </div>
    </div>
  );
}
