// SPDX-License-Identifier: Apache-2.0
// Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md.

import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./src/tracing/**/*.{ts,tsx,css}"],
  darkMode: "class",
  theme: { extend: {} },
};

export default config;
