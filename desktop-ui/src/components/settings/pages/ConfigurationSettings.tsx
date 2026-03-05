export function ConfigurationSettings() {
  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-primary">Configuration</h2>
        <p className="text-[13px] text-muted mt-1">Channels, tools, and gateway settings</p>
      </div>

      <div className="space-y-4">
        <div className="bg-white/[0.04] rounded-lg border border-white/[0.08] p-4">
          <h3 className="text-[13px] font-medium text-secondary mb-3">Channels</h3>
          <p className="text-[13px] text-dim">
            Channel configuration is managed via config.json. Dashboard editing coming soon.
          </p>
        </div>

        <div className="bg-white/[0.04] rounded-lg border border-white/[0.08] p-4">
          <h3 className="text-[13px] font-medium text-secondary mb-3">Tools</h3>
          <p className="text-[13px] text-dim">
            Tool permissions and workspace restrictions. Dashboard editing coming soon.
          </p>
        </div>

        <div className="bg-white/[0.04] rounded-lg border border-white/[0.08] p-4">
          <h3 className="text-[13px] font-medium text-secondary mb-3">Gateway</h3>
          <p className="text-[13px] text-dim">
            HTTP gateway configuration. Dashboard editing coming soon.
          </p>
        </div>
      </div>
    </div>
  );
}
