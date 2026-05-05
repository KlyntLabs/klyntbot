import { type QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { type ReactNode, useEffect, useRef } from "react";
import { startAppServerEventBridge } from "./appServerEventBridge";
import { createQueryClient } from "./client";
import { startTauriEventBridge } from "./tauriEventBridge";

interface QueryProviderProps {
  children: ReactNode;
  /** For tests: inject a pre-configured client. */
  client?: QueryClient;
}

export function QueryProvider({ children, client }: QueryProviderProps) {
  // One client per component instance (per webview React tree).
  const clientRef = useRef<QueryClient | undefined>(undefined);
  if (!clientRef.current) clientRef.current = client ?? createQueryClient();

  useEffect(() => {
    let stopTauri: (() => void) | null = null;
    let stopApp: (() => void) | null = null;
    let cancelled = false;

    const currentClient = clientRef.current;
    if (!currentClient) {
      return;
    }

    startTauriEventBridge(currentClient).then((s) => {
      if (cancelled) s();
      else stopTauri = s;
    });
    stopApp = startAppServerEventBridge(currentClient);

    return () => {
      cancelled = true;
      stopTauri?.();
      stopApp?.();
    };
  }, []);

  return (
    <QueryClientProvider client={clientRef.current}>
      {children}
      {import.meta.env.DEV && (
        <ReactQueryDevtools initialIsOpen={false} buttonPosition="bottom-left" />
      )}
    </QueryClientProvider>
  );
}
