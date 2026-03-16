import { createContext, useCallback, useContext, useState } from "react";

const PortalContainerContext = createContext<HTMLDivElement | null>(null);

export function usePortalContainer() {
  return useContext(PortalContainerContext);
}

export function PortalContainerProvider({ children }: { children: React.ReactNode }) {
  const [container, setContainer] = useState<HTMLDivElement | null>(null);
  const ref = useCallback((node: HTMLDivElement | null) => {
    if (node) setContainer(node);
  }, []);
  return (
    <PortalContainerContext.Provider value={container}>
      {children}
      <div ref={ref} className="tasks-scope" />
    </PortalContainerContext.Provider>
  );
}
