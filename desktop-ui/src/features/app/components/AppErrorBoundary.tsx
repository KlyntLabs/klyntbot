import { Component, useState, type ErrorInfo, type ReactNode } from "react";

interface AppErrorBoundaryProps {
  children: ReactNode;
  surface?: string;
}

interface AppErrorBoundaryState {
  error: unknown;
  componentStack: string | null;
}

const INITIAL_STATE: AppErrorBoundaryState = { error: null, componentStack: null };

export class AppErrorBoundary extends Component<AppErrorBoundaryProps, AppErrorBoundaryState> {
  state: AppErrorBoundaryState = INITIAL_STATE;

  static getDerivedStateFromError(error: unknown): Partial<AppErrorBoundaryState> {
    return { error };
  }

  componentDidCatch(error: unknown, info: ErrorInfo): void {
    this.setState({ componentStack: info.componentStack ?? null });
    console.error("[AppErrorBoundary]", this.props.surface ?? "Klynt", error, info);
  }

  resetError = (): void => {
    this.setState(INITIAL_STATE);
  };

  render(): ReactNode {
    if (this.state.error !== null && this.state.error !== undefined) {
      return (
        <AppErrorBoundaryFallback
          surface={this.props.surface ?? "Klynt"}
          error={this.state.error}
          componentStack={this.state.componentStack}
          resetError={this.resetError}
        />
      );
    }
    return this.props.children;
  }
}

interface FallbackProps {
  error: unknown;
  componentStack: string | null;
  resetError: () => void;
  surface: string;
}

function AppErrorBoundaryFallback({ surface, error, componentStack, resetError }: FallbackProps) {
  const [copied, setCopied] = useState(false);
  const message = error instanceof Error ? error.message : String(error);
  const stack = error instanceof Error ? error.stack ?? "" : "";

  const copyDetails = async () => {
    const payload = [
      `Surface: ${surface}`,
      `Message: ${message}`,
      stack ? `\nStack:\n${stack}` : null,
      componentStack ? `\nComponent stack:\n${componentStack}` : null,
    ]
      .filter(Boolean)
      .join("\n");
    try {
      await navigator.clipboard.writeText(payload);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard write can fail in restricted contexts — non-fatal.
    }
  };

  return (
    <div className="app-error-boundary" role="alert" aria-live="assertive">
      <div className="app-error-boundary__card">
        <div className="app-error-boundary__icon" aria-hidden="true">
          ⚠
        </div>
        <h1 className="app-error-boundary__title">Something went wrong</h1>
        <p className="app-error-boundary__subtitle">
          The {surface} window hit an unrecoverable error and was prevented from disappearing.
        </p>
        <pre className="app-error-boundary__message">{message}</pre>
        <div className="app-error-boundary__actions">
          <button
            type="button"
            className="primary app-error-boundary__button"
            onClick={() => window.location.reload()}
          >
            Reload app
          </button>
          <button
            type="button"
            className="secondary app-error-boundary__button"
            onClick={resetError}
          >
            Try again
          </button>
          <button
            type="button"
            className="ghost app-error-boundary__button"
            onClick={copyDetails}
          >
            {copied ? "Copied" : "Copy details"}
          </button>
        </div>
      </div>
    </div>
  );
}
