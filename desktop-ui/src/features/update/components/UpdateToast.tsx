import { openUrl } from "@tauri-apps/plugin-opener";
import ReactMarkdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import {
  ToastActions,
  ToastBody,
  ToastCard,
  ToastError,
  ToastHeader,
  ToastTitle,
  ToastViewport,
} from "@/features/design-system/components/toast/ToastPrimitives";
import type { PostUpdateNoticeState, UpdateState } from "../hooks/useUpdater";

type UpdateToastProps = {
  state: UpdateState;
  onUpdate: () => void;
  onDismiss: () => void;
  postUpdateNotice?: PostUpdateNoticeState;
  onDismissPostUpdateNotice?: () => void;
};

function formatBytes(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB"];
  let size = value;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[unitIndex]}`;
}

export function UpdateToast({
  state,
  onUpdate,
  onDismiss,
  postUpdateNotice = null,
  onDismissPostUpdateNotice,
}: UpdateToastProps) {
  if (postUpdateNotice) {
    return (
      <ToastViewport
        className="absolute bottom-9 right-5 w-[min(360px,calc(100vw-40px))] z-[5]"
        role="region"
        ariaLive="polite"
      >
        <ToastCard className="[--ds-toast-enter-duration:0.2s]" role="status">
          <ToastHeader className="mb-1.5">
            <ToastTitle className="text-ui-sm tracking-widest uppercase">What&apos;s New</ToastTitle>
            <div className="text-ui-sm text-text-faint">v{postUpdateNotice.version}</div>
          </ToastHeader>
          {postUpdateNotice.stage === "loading" ? (
            <ToastBody className="text-ui-sm mb-2.5">
              Updated successfully. Loading release notes...
            </ToastBody>
          ) : null}
          {postUpdateNotice.stage === "ready" ? (
            <>
              <ToastBody className="text-ui-sm mb-2.5">
                Updated successfully. Here is what is new:
              </ToastBody>
              <div className="text-ui-sm text-text-muted mb-2.5 max-h-[40vh] overflow-y-auto" role="document">
                <ReactMarkdown
                  remarkPlugins={[remarkGfm]}
                  rehypePlugins={[rehypeSanitize]}
                  components={{
                    a: ({ href, children }) => {
                      if (!href) {
                        return <span>{children}</span>;
                      }
                      return (
                        <a
                          href={href}
                          target="_blank"
                          rel="noreferrer"
                          onClick={(event) => {
                            event.preventDefault();
                            void openUrl(href);
                          }}
                        >
                          {children}
                        </a>
                      );
                    },
                  }}
                >
                  {postUpdateNotice.body}
                </ReactMarkdown>
              </div>
            </>
          ) : null}
          {postUpdateNotice.stage === "fallback" ? (
            <ToastBody className="text-ui-sm mb-2.5">
              Updated to v{postUpdateNotice.version}. Release notes could not be loaded.
            </ToastBody>
          ) : null}
          <ToastActions className="flex gap-2 justify-end flex-wrap">
            {postUpdateNotice.stage !== "loading" ? (
              <button
                type="button"
                className="primary"
                onClick={() => {
                  void openUrl(postUpdateNotice.htmlUrl);
                }}
              >
                View on GitHub
              </button>
            ) : null}
            <button
              type="button"
              className="secondary"
              onClick={onDismissPostUpdateNotice ?? onDismiss}
            >
              Dismiss
            </button>
          </ToastActions>
        </ToastCard>
      </ToastViewport>
    );
  }

  if (state.stage === "idle") {
    return null;
  }

  const totalBytes = state.progress?.totalBytes;
  const downloadedBytes = state.progress?.downloadedBytes ?? 0;
  const percent =
    totalBytes && totalBytes > 0 ? Math.min(100, (downloadedBytes / totalBytes) * 100) : null;

  return (
    <ToastViewport
      className="absolute bottom-9 right-5 w-[min(360px,calc(100vw-40px))] z-[5]"
      role="region"
      ariaLive="polite"
    >
      <ToastCard className="[--ds-toast-enter-duration:0.2s]" role="status">
        <ToastHeader className="mb-1.5">
          <ToastTitle className="text-ui-sm tracking-widest uppercase">Update</ToastTitle>
          {state.version ? <div className="text-ui-sm text-text-faint">v{state.version}</div> : null}
        </ToastHeader>
        {state.stage === "checking" && (
          <ToastBody className="text-ui-sm mb-2.5">Checking for updates...</ToastBody>
        )}
        {state.stage === "available" && (
          <>
            <ToastBody className="text-ui-sm mb-2.5">A new version is available.</ToastBody>
            <ToastActions className="flex gap-2 justify-end flex-wrap">
              <button type="button" className="secondary" onClick={onDismiss}>
                Later
              </button>
              <button type="button" className="primary" onClick={onUpdate}>
                Update
              </button>
            </ToastActions>
          </>
        )}
        {state.stage === "latest" && (
          <div className="flex items-center gap-2">
            <ToastBody className="text-ui-sm mb-0">You&apos;re up to date.</ToastBody>
            <button type="button" className="secondary" onClick={onDismiss}>
              Dismiss
            </button>
          </div>
        )}
        {state.stage === "downloading" && (
          <>
            <ToastBody className="text-ui-sm mb-2.5">Downloading update…</ToastBody>
            <div className="grid gap-1.5 mb-1">
              <div className="h-1.5 rounded-full bg-surface-card-muted overflow-hidden">
                <span
                  className="block h-full bg-gradient-to-r from-[#4fb8ff] to-[#3be082]"
                  style={{ width: percent ? `${percent}%` : "24%" }}
                />
              </div>
              <div className="text-ui-xs text-text-muted">
                {totalBytes
                  ? `${formatBytes(downloadedBytes)} / ${formatBytes(totalBytes)}`
                  : `${formatBytes(downloadedBytes)} downloaded`}
              </div>
            </div>
          </>
        )}
        {state.stage === "installing" && (
          <ToastBody className="text-ui-sm mb-2.5">Installing update…</ToastBody>
        )}
        {state.stage === "restarting" && (
          <ToastBody className="text-ui-sm mb-2.5">Restarting…</ToastBody>
        )}
        {state.stage === "error" && (
          <>
            <ToastBody className="text-ui-sm mb-2.5">Update failed.</ToastBody>
            {state.error ? (
              <ToastError className="mb-2.5">{state.error}</ToastError>
            ) : null}
            <ToastActions className="flex gap-2 justify-end flex-wrap">
              <button type="button" className="secondary" onClick={onDismiss}>
                Dismiss
              </button>
              <button type="button" className="primary" onClick={onUpdate}>
                Retry
              </button>
            </ToastActions>
          </>
        )}
      </ToastCard>
    </ToastViewport>
  );
}
