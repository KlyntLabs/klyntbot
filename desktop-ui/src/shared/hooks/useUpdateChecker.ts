import { checkUpdateAvailable, installUpdate } from "@shared/lib/updater";
import { useCallback, useEffect, useState } from "react";

export function useUpdateChecker() {
  const [updateAvailable, setUpdateAvailable] = useState(false);
  const [version, setVersion] = useState<string>();

  useEffect(() => {
    const timer = setTimeout(async () => {
      const result = await checkUpdateAvailable();
      setUpdateAvailable(result.available);
      setVersion(result.version);
    }, 3000);
    return () => clearTimeout(timer);
  }, []);

  const triggerInstall = useCallback(async () => {
    await installUpdate();
  }, []);

  return { updateAvailable, version, triggerInstall };
}
