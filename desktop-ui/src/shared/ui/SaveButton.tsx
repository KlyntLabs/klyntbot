import { Save } from "lucide-react";
import { Button, type ButtonProps } from "./Button";

export interface SaveButtonProps extends Omit<ButtonProps, "children"> {
  saving: boolean;
}

export function SaveButton({ saving, disabled, ...props }: SaveButtonProps) {
  return (
    <Button variant="primary" size="sm" disabled={saving || disabled} {...props}>
      <Save className="size-3" />
      {saving ? "Saving..." : "Save"}
    </Button>
  );
}
