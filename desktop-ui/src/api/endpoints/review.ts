import { invoke } from "@/api/client";
import type { ReviewResult } from "@/bindings";

export async function startReview(
  threadId: string,
  target: string | null,
  delivery: "inline" | "detached" = "inline",
): Promise<ReviewResult> {
  return invoke<ReviewResult>("coding_review_start", { threadId, target, delivery });
}
