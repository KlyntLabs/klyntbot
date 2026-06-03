import ChevronDown from "lucide-react/dist/esm/icons/chevron-down";
import ChevronUp from "lucide-react/dist/esm/icons/chevron-up";
import ImagePlus from "lucide-react/dist/esm/icons/image-plus";
import Mic from "lucide-react/dist/esm/icons/mic";
import Plus from "lucide-react/dist/esm/icons/plus";
import Square from "lucide-react/dist/esm/icons/square";
import X from "lucide-react/dist/esm/icons/x";
import type { Dispatch, RefObject, SetStateAction } from "react";
import { cn } from "@/utils/cn";
import {
  PopoverMenuItem,
  PopoverSurface,
} from "@/features/design-system/components/popover/PopoverPrimitives";

type ComposerMobileActionsMenuProps = {
  disabled: boolean;
  handleMobileAttachClick: () => void;
  handleMobileDictationClick: () => void;
  handleMobileExpandClick: () => void;
  isDictating: boolean;
  isDictationProcessing: boolean;
  isExpanded: boolean;
  micAriaLabel: string;
  micDisabled: boolean;
  mobileActionsOpen: boolean;
  mobileActionsRef: RefObject<HTMLDivElement | null>;
  onAddAttachment?: () => void;
  onToggleExpand?: () => void;
  setMobileActionsOpen: Dispatch<SetStateAction<boolean>>;
  showDictationAction: boolean;
};

export function ComposerMobileActionsMenu({
  disabled,
  handleMobileAttachClick,
  handleMobileDictationClick,
  handleMobileExpandClick,
  isDictating,
  isDictationProcessing,
  isExpanded,
  micAriaLabel,
  micDisabled,
  mobileActionsOpen,
  mobileActionsRef,
  onAddAttachment,
  onToggleExpand,
  setMobileActionsOpen,
  showDictationAction,
}: ComposerMobileActionsMenuProps) {
  return (
    <div
      className={cn("composer-mobile-menu relative", mobileActionsOpen && "is-open")}
      ref={mobileActionsRef}
    >
      <button
        type="button"
        className="composer-action hidden max-[720px]:inline-flex items-center justify-center w-[30px] h-[30px] rounded-full border border-[var(--cm-border-emphasis)] bg-[var(--cm-surface-panel-strong)] text-text-strong text-ui-sm p-0 cursor-pointer relative"
        onClick={() => setMobileActionsOpen((prev) => !prev)}
        disabled={disabled}
        aria-expanded={mobileActionsOpen}
        aria-haspopup="menu"
        aria-label="More actions"
        title="More actions"
      >
        <Plus size={14} aria-hidden />
      </button>
      {mobileActionsOpen && (
        <PopoverSurface className="absolute left-0 bottom-[calc(100%+8px)] min-w-[170px] p-[6px] grid gap-1 z-30" role="menu">
          <PopoverMenuItem
            onClick={handleMobileAttachClick}
            disabled={disabled || !onAddAttachment}
            icon={<ImagePlus size={14} />}
          >
            Add image
          </PopoverMenuItem>
          {onToggleExpand && (
            <PopoverMenuItem
              onClick={handleMobileExpandClick}
              disabled={disabled}
              icon={isExpanded ? <ChevronDown size={14} /> : <ChevronUp size={14} />}
            >
              {isExpanded ? "Collapse input" : "Expand input"}
            </PopoverMenuItem>
          )}
          {showDictationAction && (
            <PopoverMenuItem
              onClick={handleMobileDictationClick}
              disabled={micDisabled}
              icon={
                isDictationProcessing ? (
                  <X size={14} />
                ) : isDictating ? (
                  <Square size={14} />
                ) : (
                  <Mic size={14} />
                )
              }
            >
              {micAriaLabel}
            </PopoverMenuItem>
          )}
        </PopoverSurface>
      )}
    </div>
  );
}
