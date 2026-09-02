import { Container } from "lucide-react";

export function EnvironmentsSettings() {
  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-fg">Environments</h2>
        <p className="text-ui text-fg-secondary mt-1">Environment variable management</p>
      </div>

      <div className="island rounded-lg p-8 flex flex-col items-center text-center">
        <Container className="size-8 text-fg-dim mb-3" strokeWidth={1.5} />
        <p className="text-ui text-fg-secondary">Environment management coming soon</p>
      </div>
    </div>
  );
}
