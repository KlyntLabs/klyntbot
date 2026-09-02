import type { InputHTMLAttributes, ReactNode, TextareaHTMLAttributes } from "react";

/** Visual register for the control. */
export type FieldShape = "control" | "pill";

interface FieldSharedProps {
  /** Control id; also used for label association and described-by ids. */
  id: string;
  /** Visible label text — caller-supplied. */
  label: string;
  /** Optional helper copy announced via aria-describedby. */
  description?: ReactNode;
  /** When set, announced via role="alert" and referenced by aria-describedby. */
  error?: string;
  /** Visual register. Defaults to `control`. */
  shape?: FieldShape;
  /** Overrides the label's default classes (merged via twMerge). */
  labelClassName?: string;
  /** Overrides the wrapper's default classes (merged via twMerge). */
  wrapperClassName?: string;
}

export interface FieldProps
  extends FieldSharedProps,
    Omit<InputHTMLAttributes<HTMLInputElement>, "id"> {
  multiline?: false;
}

export interface FieldTextareaProps
  extends FieldSharedProps,
    Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "id"> {
  multiline: true;
}
