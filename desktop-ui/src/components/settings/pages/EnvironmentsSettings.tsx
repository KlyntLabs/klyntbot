import { Container } from "lucide-react";

export function EnvironmentsSettings() {
  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-primary">Environments</h2>
        <p className="text-[13px] text-muted mt-1">Environment variable management</p>
      </div>

      <div className="bg-surface-low rounded-lg border border-border p-8 flex flex-col items-center text-center">
        <Container className="w-8 h-8 text-dim mb-3" strokeWidth={1.5} />
        <p className="text-[13px] text-muted">Environment management coming soon</p>
      </div>
    </div>
  );
}
