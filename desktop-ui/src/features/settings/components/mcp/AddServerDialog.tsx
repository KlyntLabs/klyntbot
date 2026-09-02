import type { McpAddServerParams, RecommendedMcpServer } from "@shared/types";
import { Minus, Plus, X } from "lucide-react";
import { useState } from "react";

interface AddServerDialogProps {
  open: boolean;
  onClose: () => void;
  onAdd: (params: McpAddServerParams) => void;
  prefill?: RecommendedMcpServer;
}

export function AddServerDialog({ open, onClose, onAdd, prefill }: AddServerDialogProps) {
  const [name, setName] = useState(prefill?.name ?? "");
  const [transport, setTransport] = useState<"stdio" | "http">(prefill?.transport ?? "stdio");
  const [command, setCommand] = useState(prefill?.command ?? "");
  const [args, setArgs] = useState(prefill?.args?.join(", ") ?? "");
  const [envIdCounter, setEnvIdCounter] = useState(prefill?.envKeys?.length ?? 0);
  const [envPairs, setEnvPairs] = useState<{ id: number; key: string; value: string }[]>(
    prefill?.envKeys?.map((k, i) => ({ id: i, key: k, value: "" })) ?? [],
  );
  const [url, setUrl] = useState("");
  const [headerIdCounter, setHeaderIdCounter] = useState(0);
  const [headerPairs, setHeaderPairs] = useState<{ id: number; key: string; value: string }[]>([]);

  if (!open) return null;

  const handleSubmit = () => {
    const params: McpAddServerParams = {
      name: name.trim(),
      transport,
    };

    if (transport === "stdio") {
      params.command = command.trim();
      params.args = args
        .split(",")
        .map((a) => a.trim())
        .filter(Boolean);
      const env: Record<string, string> = {};
      envPairs.forEach((p) => {
        if (p.key.trim()) env[p.key.trim()] = p.value;
      });
      if (Object.keys(env).length > 0) params.env = env;
    } else {
      params.url = url.trim();
      const headers: Record<string, string> = {};
      headerPairs.forEach((p) => {
        if (p.key.trim()) headers[p.key.trim()] = p.value;
      });
      if (Object.keys(headers).length > 0) params.headers = headers;
    }

    onAdd(params);
  };

  const canSubmit =
    name.trim() !== "" && (transport === "stdio" ? command.trim() !== "" : url.trim() !== "");

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-overlay-heavy"
      role="dialog"
      aria-modal="true"
      aria-labelledby="add-server-dialog-title"
    >
      <div className="glass-panel w-[480px] max-h-[80vh] overflow-y-auto">
        <div className="island rounded-[calc(var(--ds-radius-card) - var(--ds-space-1-5))]">
          {/* Header */}
          <div className="flex items-center justify-between px-5 py-4 border-b border-separator">
            <h3 id="add-server-dialog-title" className="text-sm font-medium text-fg">
              {prefill ? `Install ${prefill.name}` : "Add MCP server"}
            </h3>
            <button
              type="button"
              onClick={onClose}
              aria-label="Close dialog"
              className="size-7 rounded-md flex items-center justify-center text-fg-secondary hover:text-fg hover:bg-control-hover transition-colors"
            >
              <X className="size-4" />
            </button>
          </div>

          {/* Body */}
          <div className="px-5 py-4 space-y-4">
            {/* Name */}
            <div>
              <label
                htmlFor="mcp-server-name"
                className="block text-ui-xs text-fg-secondary mb-1.5"
              >
                Server name
              </label>
              <input
                id="mcp-server-name"
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. my-server"
                className="w-full px-3 py-1.5 text-ui bg-control-hover border border-separator rounded-control text-fg placeholder:text-fg-dim focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator"
              />
            </div>

            {/* Transport */}
            <div>
              <span className="block text-ui-xs text-fg-secondary mb-1.5">Transport</span>
              <div className="flex gap-2">
                {(["stdio", "http"] as const).map((t) => (
                  <button
                    type="button"
                    key={t}
                    onClick={() => setTransport(t)}
                    className={`flex-1 py-1.5 text-ui-sm rounded-md border transition-colors ${
                      transport === t
                        ? "border-brand/50 text-brand bg-brand/5"
                        : "border-separator text-fg-secondary bg-control-hover hover:bg-control-hover"
                    }`}
                  >
                    {t === "stdio" ? "Stdio" : "HTTP"}
                  </button>
                ))}
              </div>
            </div>

            {/* Stdio fields */}
            {transport === "stdio" && (
              <>
                <div>
                  <label
                    htmlFor="mcp-command"
                    className="block text-ui-xs text-fg-secondary mb-1.5"
                  >
                    Command
                  </label>
                  <input
                    id="mcp-command"
                    type="text"
                    value={command}
                    onChange={(e) => setCommand(e.target.value)}
                    placeholder="e.g. npx"
                    className="w-full px-3 py-1.5 text-ui bg-control-hover border border-separator rounded-control text-fg placeholder:text-fg-dim focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator"
                  />
                </div>

                <div>
                  <label htmlFor="mcp-args" className="block text-ui-xs text-fg-secondary mb-1.5">
                    Arguments <span className="text-fg-dim">(comma-separated)</span>
                  </label>
                  <input
                    id="mcp-args"
                    type="text"
                    value={args}
                    onChange={(e) => setArgs(e.target.value)}
                    placeholder="e.g. -y, @modelcontextprotocol/server-github"
                    className="w-full px-3 py-1.5 text-ui bg-control-hover border border-separator rounded-control text-fg placeholder:text-fg-dim focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator"
                  />
                </div>

                {/* Env key-value pairs */}
                <div>
                  <div className="flex items-center justify-between mb-1.5">
                    <span className="text-ui-xs text-fg-secondary">Environment variables</span>
                    <button
                      type="button"
                      onClick={() => {
                        setEnvPairs([...envPairs, { id: envIdCounter, key: "", value: "" }]);
                        setEnvIdCounter((c) => c + 1);
                      }}
                      aria-label="Add environment variable"
                      className="size-5 rounded flex items-center justify-center text-fg-secondary hover:text-fg hover:bg-control-hover transition-colors"
                    >
                      <Plus className="size-3.5" />
                    </button>
                  </div>
                  <div className="space-y-1.5">
                    {envPairs.map((pair, i) => (
                      <div key={pair.id} className="flex gap-1.5">
                        <input
                          type="text"
                          value={pair.key}
                          onChange={(e) => {
                            const next = [...envPairs];
                            next[i] = { ...next[i], key: e.target.value };
                            setEnvPairs(next);
                          }}
                          placeholder="KEY"
                          className="flex-1 px-2.5 py-1.5 text-ui-sm font-mono bg-control-hover border border-separator rounded-control text-fg placeholder:text-fg-dim focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator"
                        />
                        <input
                          type="text"
                          value={pair.value}
                          onChange={(e) => {
                            const next = [...envPairs];
                            next[i] = { ...next[i], value: e.target.value };
                            setEnvPairs(next);
                          }}
                          placeholder="value"
                          className="flex-1 px-2.5 py-1.5 text-ui-sm font-mono bg-control-hover border border-separator rounded-control text-fg placeholder:text-fg-dim focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator"
                        />
                        <button
                          type="button"
                          onClick={() => setEnvPairs(envPairs.filter((_, j) => j !== i))}
                          aria-label="Remove environment variable"
                          className="size-7 rounded flex items-center justify-center text-fg-secondary hover:text-status-danger hover:bg-control-hover transition-colors flex-shrink-0"
                        >
                          <Minus className="size-3.5" />
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              </>
            )}

            {/* HTTP fields */}
            {transport === "http" && (
              <>
                <div>
                  <label htmlFor="mcp-url" className="block text-ui-xs text-fg-secondary mb-1.5">
                    URL
                  </label>
                  <input
                    id="mcp-url"
                    type="url"
                    value={url}
                    onChange={(e) => setUrl(e.target.value)}
                    placeholder="e.g. https://mcp.example.com/v1"
                    className="w-full px-3 py-1.5 text-ui bg-control-hover border border-separator rounded-control text-fg placeholder:text-fg-dim focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator"
                  />
                </div>

                {/* Header key-value pairs */}
                <div>
                  <div className="flex items-center justify-between mb-1.5">
                    <span className="text-ui-xs text-fg-secondary">Headers</span>
                    <button
                      type="button"
                      onClick={() => {
                        setHeaderPairs([
                          ...headerPairs,
                          { id: headerIdCounter, key: "", value: "" },
                        ]);
                        setHeaderIdCounter((c) => c + 1);
                      }}
                      aria-label="Add header"
                      className="size-5 rounded flex items-center justify-center text-fg-secondary hover:text-fg hover:bg-control-hover transition-colors"
                    >
                      <Plus className="size-3.5" />
                    </button>
                  </div>
                  <div className="space-y-1.5">
                    {headerPairs.map((pair, i) => (
                      <div key={pair.id} className="flex gap-1.5">
                        <input
                          type="text"
                          value={pair.key}
                          onChange={(e) => {
                            const next = [...headerPairs];
                            next[i] = { ...next[i], key: e.target.value };
                            setHeaderPairs(next);
                          }}
                          placeholder="Header-Name"
                          className="flex-1 px-2.5 py-1.5 text-ui-sm font-mono bg-control-hover border border-separator rounded-control text-fg placeholder:text-fg-dim focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator"
                        />
                        <input
                          type="text"
                          value={pair.value}
                          onChange={(e) => {
                            const next = [...headerPairs];
                            next[i] = { ...next[i], value: e.target.value };
                            setHeaderPairs(next);
                          }}
                          placeholder="value"
                          className="flex-1 px-2.5 py-1.5 text-ui-sm font-mono bg-control-hover border border-separator rounded-control text-fg placeholder:text-fg-dim focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator"
                        />
                        <button
                          type="button"
                          onClick={() => setHeaderPairs(headerPairs.filter((_, j) => j !== i))}
                          aria-label="Remove header"
                          className="size-7 rounded flex items-center justify-center text-fg-secondary hover:text-status-danger hover:bg-control-hover transition-colors flex-shrink-0"
                        >
                          <Minus className="size-3.5" />
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              </>
            )}
          </div>

          {/* Footer */}
          <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-separator">
            <button
              type="button"
              onClick={onClose}
              className="px-3 py-1.5 text-ui-xs text-fg-secondary hover:text-fg rounded-md hover:bg-control-hover transition-colors"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleSubmit}
              disabled={!canSubmit}
              className="px-4 py-1.5 text-ui-sm rounded-md bg-brand text-white hover:bg-brand-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              {prefill ? "Install" : "Add server"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
