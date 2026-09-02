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
          <AlertTriangle className="size-8 text-fg-secondary" />
          <p className="text-sm text-fg-secondary">Failed to load issue detail</p>
          <button
            type="button"
            onClick={() => this.setState({ error: null })}
            className="text-ui-sm px-3 py-1.5 rounded border border-separator text-fg hover:bg-control-hover transition-colors"
          >
            Retry
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
