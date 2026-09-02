import { GitBranch } from "lucide-react";

export function GitSettings() {
  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-fg">Git</h2>
        <p className="text-ui text-fg-secondary mt-1">Version control integration</p>
      </div>

      <div className="island rounded-lg p-8 flex flex-col items-center text-center">
        <GitBranch className="size-8 text-fg-dim mb-3" strokeWidth={1.5} />
        <p className="text-ui text-fg-secondary">Git integration coming soon</p>
      </div>
    </div>
  );
}
