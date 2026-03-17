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
          <AlertTriangle className="size-8 text-muted" />
          <p className="text-sm text-muted">Failed to load issue detail</p>
          <button
            type="button"
            onClick={() => this.setState({ error: null })}
            className="text-xs px-3 py-1.5 rounded border border-border text-primary hover:bg-surface-raised transition-colors"
          >
            Retry
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
