import { Minus, Plus, X } from "lucide-react";
import { useState } from "react";
import type { McpAddServerParams, RecommendedMcpServer } from "../../../lib/types";

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
  const [envPairs, setEnvPairs] = useState<{ key: string; value: string }[]>(
    prefill?.envKeys?.map((k) => ({ key: k, value: "" })) ?? [],
  );
  const [url, setUrl] = useState("");
  const [headerPairs, setHeaderPairs] = useState<{ key: string; value: string }[]>([]);

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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="glass-panel w-[480px] max-h-[80vh] overflow-y-auto">
        <div className="bg-surface-low rounded-[var(--glass-radius-inner)]">
          {/* Header */}
          <div className="flex items-center justify-between px-5 py-4 border-b border-border">
            <h3 className="text-[14px] font-medium text-primary">
              {prefill ? `Install ${prefill.name}` : "Add MCP server"}
            </h3>
            <button
              onClick={onClose}
              className="w-7 h-7 rounded-md flex items-center justify-center text-muted hover:text-secondary hover:bg-surface-base transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          {/* Body */}
          <div className="px-5 py-4 space-y-4">
            {/* Name */}
            <div>
              <label className="block text-[12px] text-muted mb-1.5">Server name</label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. my-server"
                className="w-full px-3 py-1.5 text-[13px] bg-surface-base border border-border rounded-md text-primary placeholder:text-dim focus:outline-none focus:border-brand/50"
              />
            </div>

            {/* Transport */}
            <div>
              <label className="block text-[12px] text-muted mb-1.5">Transport</label>
              <div className="flex gap-2">
                {(["stdio", "http"] as const).map((t) => (
                  <button
                    key={t}
                    onClick={() => setTransport(t)}
                    className={`flex-1 py-1.5 text-[12px] rounded-md border transition-colors ${
                      transport === t
                        ? "border-brand/50 text-brand bg-brand/5"
                        : "border-border text-muted bg-surface-base hover:bg-surface-raised"
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
                  <label className="block text-[12px] text-muted mb-1.5">Command</label>
                  <input
                    type="text"
                    value={command}
                    onChange={(e) => setCommand(e.target.value)}
                    placeholder="e.g. npx"
                    className="w-full px-3 py-1.5 text-[13px] bg-surface-base border border-border rounded-md text-primary placeholder:text-dim focus:outline-none focus:border-brand/50"
                  />
                </div>

                <div>
                  <label className="block text-[12px] text-muted mb-1.5">
                    Arguments <span className="text-dim">(comma-separated)</span>
                  </label>
                  <input
                    type="text"
                    value={args}
                    onChange={(e) => setArgs(e.target.value)}
                    placeholder="e.g. -y, @modelcontextprotocol/server-github"
                    className="w-full px-3 py-1.5 text-[13px] bg-surface-base border border-border rounded-md text-primary placeholder:text-dim focus:outline-none focus:border-brand/50"
                  />
                </div>

                {/* Env key-value pairs */}
                <div>
                  <div className="flex items-center justify-between mb-1.5">
                    <label className="text-[12px] text-muted">Environment variables</label>
                    <button
                      onClick={() => setEnvPairs([...envPairs, { key: "", value: "" }])}
                      className="w-5 h-5 rounded flex items-center justify-center text-muted hover:text-secondary hover:bg-surface-base transition-colors"
                    >
                      <Plus className="w-3.5 h-3.5" />
                    </button>
                  </div>
                  <div className="space-y-1.5">
                    {envPairs.map((pair, i) => (
                      <div key={i} className="flex gap-1.5">
                        <input
                          type="text"
                          value={pair.key}
                          onChange={(e) => {
                            const next = [...envPairs];
                            next[i] = { ...next[i], key: e.target.value };
                            setEnvPairs(next);
                          }}
                          placeholder="KEY"
                          className="flex-1 px-2.5 py-1.5 text-[12px] font-mono bg-surface-base border border-border rounded-md text-primary placeholder:text-dim focus:outline-none focus:border-brand/50"
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
                          className="flex-1 px-2.5 py-1.5 text-[12px] font-mono bg-surface-base border border-border rounded-md text-primary placeholder:text-dim focus:outline-none focus:border-brand/50"
                        />
                        <button
                          onClick={() => setEnvPairs(envPairs.filter((_, j) => j !== i))}
                          className="w-7 h-7 rounded flex items-center justify-center text-muted hover:text-destructive hover:bg-surface-base transition-colors flex-shrink-0"
                        >
                          <Minus className="w-3.5 h-3.5" />
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
                  <label className="block text-[12px] text-muted mb-1.5">URL</label>
                  <input
                    type="text"
                    value={url}
                    onChange={(e) => setUrl(e.target.value)}
                    placeholder="e.g. https://mcp.example.com/v1"
                    className="w-full px-3 py-1.5 text-[13px] bg-surface-base border border-border rounded-md text-primary placeholder:text-dim focus:outline-none focus:border-brand/50"
                  />
                </div>

                {/* Header key-value pairs */}
                <div>
                  <div className="flex items-center justify-between mb-1.5">
                    <label className="text-[12px] text-muted">Headers</label>
                    <button
                      onClick={() => setHeaderPairs([...headerPairs, { key: "", value: "" }])}
                      className="w-5 h-5 rounded flex items-center justify-center text-muted hover:text-secondary hover:bg-surface-base transition-colors"
                    >
                      <Plus className="w-3.5 h-3.5" />
                    </button>
                  </div>
                  <div className="space-y-1.5">
                    {headerPairs.map((pair, i) => (
                      <div key={i} className="flex gap-1.5">
                        <input
                          type="text"
                          value={pair.key}
                          onChange={(e) => {
                            const next = [...headerPairs];
                            next[i] = { ...next[i], key: e.target.value };
                            setHeaderPairs(next);
                          }}
                          placeholder="Header-Name"
                          className="flex-1 px-2.5 py-1.5 text-[12px] font-mono bg-surface-base border border-border rounded-md text-primary placeholder:text-dim focus:outline-none focus:border-brand/50"
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
                          className="flex-1 px-2.5 py-1.5 text-[12px] font-mono bg-surface-base border border-border rounded-md text-primary placeholder:text-dim focus:outline-none focus:border-brand/50"
                        />
                        <button
                          onClick={() => setHeaderPairs(headerPairs.filter((_, j) => j !== i))}
                          className="w-7 h-7 rounded flex items-center justify-center text-muted hover:text-destructive hover:bg-surface-base transition-colors flex-shrink-0"
                        >
                          <Minus className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              </>
            )}
          </div>

          {/* Footer */}
          <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-border">
            <button
              onClick={onClose}
              className="px-3 py-1.5 text-[12px] text-muted hover:text-secondary rounded-md hover:bg-surface-base transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleSubmit}
              disabled={!canSubmit}
              className="px-4 py-1.5 text-[12px] rounded-md bg-brand text-white hover:bg-brand-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              {prefill ? "Install" : "Add server"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
