/**
 * @klyntbot/design-system public API.
 * Import only from this barrel — do not deep-import subpaths.
 */

export { cn } from "./lib/cn";
export { focusRing } from "./lib/focus";
export {
  ConfirmDialog,
  type ConfirmDialogProps,
} from "./composites/confirm-dialog";
export { Dialog, type DialogProps, type DialogSize } from "./composites/dialog";
export {
  Field,
  type FieldProps,
  type FieldShape,
  type FieldTextareaProps,
} from "./composites/field";
export { Badge, badgeVariants, type BadgeProps, type BadgeSize, type BadgeVariant } from "./primitives/badge";
export { Button, buttonVariants, type ButtonProps } from "./primitives/button";
export { Checkbox, type CheckboxProps } from "./primitives/checkbox";
export { Input, inputVariants, type InputProps } from "./primitives/input";
export { Skeleton, type SkeletonProps } from "./primitives/skeleton";
export { Spinner, type SpinnerProps } from "./primitives/spinner";
export { Toggle, type ToggleProps } from "./primitives/toggle";
export { Tooltip, type TooltipProps } from "./primitives/tooltip";
