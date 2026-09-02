import { Component, type ErrorInfo, Fragment, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  retryKey: number;
}

/**
 * Root error boundary — catches uncaught render errors and displays
 * a recovery UI instead of a blank white screen.
 */
export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, retryKey: 0 };
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Uncaught render error:", error, info.componentStack);
  }

  handleReload = () => {
    window.location.reload();
  };

  handleRetry = () => {
    this.setState((prev) => ({ hasError: false, error: null, retryKey: prev.retryKey + 1 }));
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="flex items-center justify-center min-h-screen bg-bg-elevated p-8">
          <div className="max-w-md text-center space-y-4">
            <h1 className="text-xl font-semibold text-fg">Something went wrong</h1>
            <p className="text-ui text-fg-secondary">
              An unexpected error occurred. You can try again or reload the app.
            </p>
            {this.state.error && (
              <pre className="text-ui-sm text-left bg-glass-subtle p-3 rounded-panel overflow-auto max-h-32 text-fg-secondary border border-separator">
                {this.state.error.message}
              </pre>
            )}
            <div className="flex gap-3 justify-center pt-2">
              <button
                type="button"
                onClick={this.handleRetry}
                className="px-4 py-2 text-ui rounded-control border border-separator text-fg hover:bg-glass-subtle transition-colors"
              >
                Try Again
              </button>
              <button
                type="button"
                onClick={this.handleReload}
                className="px-4 py-2 text-ui rounded-control bg-brand text-brand-foreground hover:bg-brand-hover transition-opacity"
              >
                Reload App
              </button>
            </div>
          </div>
        </div>
      );
    }

    return <Fragment key={this.state.retryKey}>{this.props.children}</Fragment>;
  }
}
