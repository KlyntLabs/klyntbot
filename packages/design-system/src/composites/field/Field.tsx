import type { InputHTMLAttributes, TextareaHTMLAttributes } from "react";
import { cn } from "../../lib/cn";
import { Input, inputVariants } from "../../primitives/input";
import type { FieldProps, FieldTextareaProps } from "./Field.types";

const fieldClass = "flex w-full flex-col gap-1.5 text-left";
const labelClass = "text-ui-sm font-medium text-fg-secondary";
const descriptionClass = "text-ui-sm text-fg-secondary";
const errorClass = "text-ui-sm text-status-danger";

function inputRest(props: FieldProps) {
  const {
    id,
    label,
    description,
    error,
    shape,
    multiline,
    className,
    labelClassName,
    wrapperClassName,
    ...rest
  } = props;
  return rest satisfies InputHTMLAttributes<HTMLInputElement>;
}

function textareaRest(props: FieldTextareaProps) {
  const {
    id,
    label,
    description,
    error,
    shape,
    multiline,
    className,
    labelClassName,
    wrapperClassName,
    ...rest
  } = props;
  return rest satisfies TextareaHTMLAttributes<HTMLTextAreaElement>;
}

/**
 * Accessible labelled form field — owns label association, description,
 * aria-invalid, aria-describedby, and the error alert region.
 */
export function Field(props: FieldProps | FieldTextareaProps) {
  const {
    id,
    label,
    description,
    error,
    shape = "control",
    className,
    labelClassName,
    wrapperClassName,
  } = props;

  const descriptionId = `${id}-description`;
  const errorId = `${id}-error`;
  const hasError = Boolean(error);
  const hasDescription = Boolean(description);
  const describedBy = [
    hasDescription ? descriptionId : null,
    hasError ? errorId : null,
  ]
    .filter(Boolean)
    .join(" ");

  const controlClassName = cn(
    shape === "pill" && "rounded-full px-3.5",
    className,
  );

  return (
    <div className={cn(fieldClass, wrapperClassName)}>
      <label htmlFor={id} className={cn(labelClass, labelClassName)}>
        {label}
      </label>
      {hasDescription ? (
        <p id={descriptionId} className={descriptionClass}>
          {description}
        </p>
      ) : null}
      {props.multiline ? (
        <textarea
          id={id}
          className={cn(inputVariants({ className: controlClassName }), "min-h-20")}
          {...textareaRest(props)}
          aria-invalid={hasError || undefined}
          aria-describedby={describedBy || undefined}
        />
      ) : (
        <Input
          id={id}
          className={cn("w-full", controlClassName)}
          {...inputRest(props)}
          aria-invalid={hasError || undefined}
          aria-describedby={describedBy || undefined}
        />
      )}
      {hasError ? (
        <p id={errorId} role="alert" className={errorClass}>
          {error}
        </p>
      ) : null}
    </div>
  );
}
