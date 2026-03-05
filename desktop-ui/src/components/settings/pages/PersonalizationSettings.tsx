export function PersonalizationSettings() {
  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-primary">Personalization</h2>
        <p className="text-[13px] text-muted mt-1">Persona and learning preferences</p>
      </div>

      <div className="space-y-4">
        <div className="bg-white/[0.04] rounded-lg border border-white/[0.08] p-4">
          <h3 className="text-[13px] font-medium text-secondary mb-3">Persona</h3>
          <p className="text-[13px] text-dim">
            Persona management and scoping. Dashboard editing coming soon.
          </p>
        </div>

        <div className="bg-white/[0.04] rounded-lg border border-white/[0.08] p-4">
          <h3 className="text-[13px] font-medium text-secondary mb-3">Learning</h3>
          <p className="text-[13px] text-dim">
            Adaptive learning thresholds and preferences. Dashboard editing coming soon.
          </p>
        </div>
      </div>
    </div>
  );
}
