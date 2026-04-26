import type { GitLayoutNodesOptions } from "./layoutNodes/buildGitNodes";
import { buildGitNodes } from "./layoutNodes/buildGitNodes";
import type { PrimaryLayoutNodesOptions } from "./layoutNodes/buildPrimaryNodes";
import { buildPrimaryNodes } from "./layoutNodes/buildPrimaryNodes";
import type { SecondaryLayoutNodesOptions } from "./layoutNodes/buildSecondaryNodes";
import { buildSecondaryNodes } from "./layoutNodes/buildSecondaryNodes";
import type { LayoutNodesOptions, LayoutNodesResult } from "./layoutNodes/types";

export function useLayoutNodes(options: LayoutNodesOptions): LayoutNodesResult {
  const primaryOptions: PrimaryLayoutNodesOptions = options.primary;
  const gitOptions: GitLayoutNodesOptions = options.git;
  const secondaryOptions: SecondaryLayoutNodesOptions = options.secondary;

  return {
    ...buildPrimaryNodes(primaryOptions),
    ...buildGitNodes(gitOptions),
    ...buildSecondaryNodes(secondaryOptions),
  };
}
