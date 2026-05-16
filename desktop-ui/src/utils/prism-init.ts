import Prism from "prismjs";

// Prism language components (prism-bash, prism-rust, …) are UMD scripts that
// reference a global `Prism`. The ESM build doesn't expose it, so we forward
// it here. This file must be imported BEFORE any `prismjs/components/*` import.
(globalThis as unknown as { Prism: typeof Prism }).Prism = Prism;

export default Prism;
