import { Container } from "lucide-react";

export function EnvironmentsSettings() {
  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-foreground">Environments</h2>
        <p className="text-[13px] text-muted-foreground mt-1">Environment variable management</p>
      </div>

      <div className="bg-card rounded-lg border border-border p-8 flex flex-col items-center text-center">
        <Container className="size-8 text-dim mb-3" strokeWidth={1.5} />
        <p className="text-[13px] text-muted-foreground">Environment management coming soon</p>
      </div>
    </div>
  );
}
