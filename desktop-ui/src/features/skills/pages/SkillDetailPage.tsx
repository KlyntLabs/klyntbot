import { ipc } from "@shared/hooks/useIpc";
import { useEffect, useState } from "react";
import { useParams } from "react-router";
import { InstallCta } from "../components/InstallCta";
import { SkillDetailSidebar } from "../components/SkillDetailSidebar";
import { SkillMarkdown } from "../components/SkillMarkdown";
import { UninstallDialog } from "../components/UninstallDialog";
import { useSkillDetail } from "../hooks/useSkillDetail";

export function SkillDetailPage() {
  const { source } = useParams<{ source: string }>();
  const decoded = source ? decodeURIComponent(source) : "";
  const [skillMd, setSkillMd] = useState<string | null>(null);
  const [loadErr, setLoadErr] = useState<string | null>(null);
  const [uninstallOpen, setUninstallOpen] = useState(false);
  const name = decoded.split("/").slice(-1)[0] ?? "";
  const { installed } = useSkillDetail(name);

  useEffect(() => {
    if (!decoded) return;
    ipc<{ package: { skillMdContent: string } }>("skill_install_preview", { shorthand: decoded })
      .then((p) => setSkillMd(p.package.skillMdContent))
      .catch((e) => setLoadErr(String(e)));
  }, [decoded]);

  return (
    <div className="flex h-full">
      <div className="flex-1 overflow-y-auto p-8 max-w-3xl">
        <p className="text-xs text-muted-foreground mb-2">Skills / {decoded}</p>
        <h1 className="text-2xl font-semibold text-foreground mb-4">{name}</h1>
        <div className="mb-6 flex gap-2">
          {installed ? (
            <>
              <span className="px-3 py-1.5 text-sm text-brand border border-brand rounded">
                Installed · v{installed.installedVersion}
              </span>
              <button
                type="button"
                onClick={() => setUninstallOpen(true)}
                className="px-3 py-1.5 text-sm text-red-400 border border-red-400 rounded"
              >
                Uninstall
              </button>
            </>
          ) : (
            <InstallCta sourceRef={decoded} />
          )}
        </div>
        {loadErr && <p className="text-sm text-red-400">{loadErr}</p>}
        {skillMd ? (
          <SkillMarkdown content={skillMd} />
        ) : (
          <p className="text-muted-foreground text-sm">Loading…</p>
        )}
      </div>
      <SkillDetailSidebar sourceRef={decoded} installed={installed} />
      {installed && uninstallOpen && (
        <UninstallDialog skill={installed} onClose={() => setUninstallOpen(false)} />
      )}
    </div>
  );
}
