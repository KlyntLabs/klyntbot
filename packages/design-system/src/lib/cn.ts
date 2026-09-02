import { type ClassValue, clsx } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

/**
 * Design-system font-size scale suffixes for `text-*` utilities.
 * Must stay in sync with `@theme inline` `--text-*` entries in styles/theme.css.
 * Without registration, tailwind-merge treats `text-ui` as a colour and silently
 * evicts it when merged with `text-fg` (and vice versa).
 */
const dsFontSizeTokens = [
  "ui-xs",
  "ui-sm",
  "ui",
  "body",
  "title-sm",
  "title",
  "title-lg",
  "display-sm",
  "display",
];

const twMerge = extendTailwindMerge({
  extend: {
    theme: {
      text: dsFontSizeTokens,
    },
  },
});

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
