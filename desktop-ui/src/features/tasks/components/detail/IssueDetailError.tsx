import { AlertTriangle } from "lucide-react";
import { Component } from "react";

interface Props {
  children: React.ReactNode;
}

interface State {
  error: Error | null;
}

export class IssueDetailErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex flex-col items-center justify-center h-full gap-3 px-6 py-8">
          <AlertTriangle className="size-8 text-[hsl(var(--muted-foreground))]" />
          <p className="text-sm text-[hsl(var(--muted-foreground))]">Failed to load issue detail</p>
          <button
            type="button"
            onClick={() => this.setState({ error: null })}
            className="text-xs px-3 py-1.5 rounded border border-[hsl(var(--border))] text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))] transition-colors"
          >
            Retry
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
