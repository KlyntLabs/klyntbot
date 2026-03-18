import { GitBranch } from "lucide-react";

export function GitSettings() {
  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-foreground">Git</h2>
        <p className="text-[13px] text-muted-foreground mt-1">Version control integration</p>
      </div>

      <div className="bg-card rounded-lg border border-border p-8 flex flex-col items-center text-center">
        <GitBranch className="w-8 h-8 text-dim mb-3" strokeWidth={1.5} />
        <p className="text-[13px] text-muted-foreground">Git integration coming soon</p>
      </div>
    </div>
  );
}
