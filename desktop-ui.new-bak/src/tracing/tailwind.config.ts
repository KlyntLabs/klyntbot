// SPDX-License-Identifier: Apache-2.0
// Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md.

import type { Config } from "tailwindcss";

// Note: in Tailwind v4 the `darkMode` config field is ignored; the dark
// variant is declared via `@custom-variant dark (...)` in tracing.css.
const config: Config = {
  content: ["./src/tracing/**/*.{ts,tsx,css}"],
  theme: { extend: {} },
};

export default config;
