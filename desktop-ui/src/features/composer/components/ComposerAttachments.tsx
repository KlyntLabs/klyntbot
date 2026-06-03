import { convertFileSrc } from "@tauri-apps/api/core";
import Image from "lucide-react/dist/esm/icons/image";
import X from "lucide-react/dist/esm/icons/x";
import { memo, useCallback } from "react";

type ComposerAttachmentsProps = {
  attachments: string[];
  disabled: boolean;
  onRemoveAttachment?: (path: string) => void;
};

function fileTitle(path: string) {
  if (path.startsWith("data:")) {
    return "Pasted image";
  }
  if (path.startsWith("http://") || path.startsWith("https://")) {
    return "Image";
  }
  const normalized = path.replace(/\\/g, "/");
  const parts = normalized.split("/").filter(Boolean);
  return parts.length ? parts[parts.length - 1] : path;
}

function attachmentPreviewSrc(path: string) {
  if (path.startsWith("data:")) {
    return path;
  }
  if (path.startsWith("http://") || path.startsWith("https://")) {
    return path;
  }
  try {
    return convertFileSrc(path);
  } catch {
    return "";
  }
}

type AttachmentItemProps = {
  path: string;
  disabled: boolean;
  onRemoveAttachment?: (path: string) => void;
};

const AttachmentItem = memo(function AttachmentItem({
  path,
  disabled,
  onRemoveAttachment,
}: AttachmentItemProps) {
  const title = fileTitle(path);
  const titleAttr = path.startsWith("data:") ? "Pasted image" : path;
  const previewSrc = attachmentPreviewSrc(path);
  const handleRemove = useCallback(() => {
    onRemoveAttachment?.(path);
  }, [onRemoveAttachment, path]);

  return (
    <div className="composer-attachment inline-flex items-center gap-1.5 py-0.5 px-2 rounded-full bg-surface-card border border-border-muted text-text-muted text-ui-xs max-w-full relative" title={titleAttr}>
      {previewSrc && (
        <span className="composer-attachment-preview absolute left-1/2 bottom-[calc(100%+8px)] w-60 h-[180px] rounded-xl overflow-hidden border border-border-subtle bg-surface-quiet shadow-xl opacity-0 -translate-x-1/2 -translate-y-[2px] scale-[0.98] pointer-events-none z-20" aria-hidden>
          <img src={previewSrc} alt="" />
        </span>
      )}
      {previewSrc ? (
        <span className="composer-attachment-thumb w-5 h-5 rounded-md overflow-hidden border border-border-subtle bg-surface-item flex-shrink-0" aria-hidden>
          <img src={previewSrc} alt="" />
        </span>
      ) : (
        <span className="composer-icon inline-flex w-3.5 h-3.5 text-text-muted" aria-hidden>
          <Image size={14} />
        </span>
      )}
      <span className="overflow-hidden text-ellipsis whitespace-nowrap max-w-[180px]">{title}</span>
      <button
        type="button"
        className="composer-attachment-remove inline-flex border-0 bg-transparent text-text-faint p-0 cursor-pointer"
        onClick={handleRemove}
        aria-label={`Remove ${title}`}
        disabled={disabled}
      >
        <X size={12} aria-hidden />
      </button>
    </div>
  );
});

export const ComposerAttachments = memo(function ComposerAttachments({
  attachments,
  disabled,
  onRemoveAttachment,
}: ComposerAttachmentsProps) {
  if (attachments.length === 0) {
    return null;
  }

  return (
    <div className="flex flex-wrap gap-2 mb-2">
      {attachments.map((path) => (
        <AttachmentItem
          key={path}
          path={path}
          disabled={disabled}
          onRemoveAttachment={onRemoveAttachment}
        />
      ))}
    </div>
  );
});
