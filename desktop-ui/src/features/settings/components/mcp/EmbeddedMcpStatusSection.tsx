import type { EmbeddedMcpStatusResponse } from "@shared/types";
import { Badge, Skeleton } from "@shared/ui";
import type { ReactNode } from "react";

type BadgeVariant = "success" | "warning" | "destructive" | "default";

function stateChip(state: string): { label: string; variant: BadgeVariant } {
  switch (state) {
    case "ready":
      return { label: "Ready", variant: "success" };
    case "disabled":
      return { label: "Disabled", variant: "default" };
    case "invalid":
      return { label: "Invalid", variant: "destructive" };
    default:
      return { label: state, variant: "warning" };
  }
}

function reasonLabel(reason: string): string {
  switch (reason) {
    case "unknown":
      return "unknown tool";
    case "forbidden":
      return "forbidden";
    default:
      return reason;
  }
}

function EmbeddedMcpShell({ children, busy }: { children: ReactNode; busy?: boolean }) {
  return (
    <section className="mb-8" aria-labelledby="embedded-mcp-heading">
      <h3 id="embedded-mcp-heading" className="text-ui font-medium text-fg-secondary mb-3">
        Embedded MCP server
      </h3>
      <div className="island rounded-lg p-4 space-y-3" aria-busy={busy || undefined}>
        {children}
      </div>
    </section>
  );
}

export interface EmbeddedMcpStatusSectionProps {
  status: EmbeddedMcpStatusResponse | null;
  loading?: boolean;
}

export function EmbeddedMcpStatusSection({ status, loading }: EmbeddedMcpStatusSectionProps) {
  if (loading && !status) {
    return (
      <EmbeddedMcpShell busy>
        <Skeleton className="h-4 w-28" />
        <Skeleton className="h-3 w-48" />
      </EmbeddedMcpShell>
    );
  }

  if (!status) {
    return (
      <EmbeddedMcpShell>
        <div className="flex items-center gap-2 min-w-0">
          <Badge variant="warning" size="sm" aria-label="Status: Unavailable">
            Unavailable
          </Badge>
          <p className="text-ui text-fg">Status could not be loaded</p>
        </div>
        <p className="text-ui-xs text-fg-dim">
          KlyntBot&apos;s in-process MCP server — separate from external client servers below.
        </p>
      </EmbeddedMcpShell>
    );
  }

  const chip = stateChip(status.state);
  const effectiveCount = status.effective.length;
  const showRejections = status.state === "invalid" || status.rejected.length > 0;

  return (
    <EmbeddedMcpShell>
      <div className="flex items-center justify-between gap-3 flex-wrap">
        <div className="flex items-center gap-2 min-w-0">
          <Badge variant={chip.variant} size="sm" aria-label={`Status: ${chip.label}`}>
            {chip.label}
          </Badge>
          <p className="text-ui text-fg">
            {status.state === "ready"
              ? `${effectiveCount} tool${effectiveCount === 1 ? "" : "s"} exposed`
              : status.state === "disabled"
                ? "Server is off in configuration"
                : "Exposure override rejected"}
          </p>
        </div>
      </div>

      <p className="text-ui-xs text-fg-dim">
        KlyntBot&apos;s in-process MCP server — separate from external client servers below.
      </p>

      {showRejections ? (
        <details className="group" open={status.state === "invalid"}>
          <summary className="cursor-pointer text-ui-sm text-fg-secondary hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-fg/30 focus-visible:ring-offset-2 focus-visible:ring-offset-transparent rounded-sm list-outside pl-1">
            Rejection summary ({status.rejected.length})
          </summary>
          {status.rejected.length === 0 ? (
            <p className="mt-2 text-ui-sm text-fg-dim">No rejected entries.</p>
          ) : (
            <ul className="mt-2 space-y-1.5">
              {status.rejected.map((entry) => (
                <li
                  key={`${entry.name}:${entry.reason}`}
                  className="flex items-baseline justify-between gap-3 text-ui-sm"
                >
                  <span className="font-mono text-ui-sm text-fg truncate">{entry.name}</span>
                  <span className="text-ui-xs text-status-danger shrink-0">
                    {reasonLabel(entry.reason)}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </details>
      ) : null}
    </EmbeddedMcpShell>
  );
}
