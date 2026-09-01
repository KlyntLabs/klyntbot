// SPDX-License-Identifier: Apache-2.0
// Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md.

import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
