import { commands } from "@/bindings";
import type { CodingTodoView, PlanModeView, TodoItem } from "@/bindings";

export type { CodingTodoView, PlanModeView, TodoItem };

export async function fetchCodingTodos(threadId: string): Promise<CodingTodoView> {
  const r = await commands.codingTodoGet(threadId);
  if (r.status !== "ok") throw new Error(r.error.message ?? "coding_todo_get failed");
  return r.data;
}

export async function enterPlanMode(threadId: string): Promise<CodingTodoView> {
  const r = await commands.codingPlanEnter(threadId);
  if (r.status !== "ok") throw new Error(r.error.message ?? "coding_plan_enter failed");
  return r.data;
}

export async function cancelPlanMode(threadId: string): Promise<CodingTodoView> {
  const r = await commands.codingPlanCancel(threadId);
  if (r.status !== "ok") throw new Error(r.error.message ?? "coding_plan_cancel failed");
  return r.data;
}

export async function ratifyPlan(
  threadId: string,
  planSessionId: string,
): Promise<CodingTodoView> {
  const r = await commands.codingPlanRatify(threadId, planSessionId);
  if (r.status !== "ok") throw new Error(r.error.message ?? "coding_plan_ratify failed");
  return r.data;
}

export async function editPlanItems(
  threadId: string,
  planSessionId: string,
  itemsJson: string,
): Promise<CodingTodoView> {
  const r = await commands.codingPlanUserEdit(threadId, planSessionId, itemsJson);
  if (r.status !== "ok") throw new Error(r.error.message ?? "coding_plan_user_edit failed");
  return r.data;
}

export async function removePlanItems(
  threadId: string,
  planSessionId: string,
  itemIds: string[],
): Promise<CodingTodoView> {
  const r = await commands.codingPlanUserRemove(threadId, planSessionId, itemIds);
  if (r.status !== "ok") throw new Error(r.error.message ?? "coding_plan_user_remove failed");
  return r.data;
}

export async function openPlanFile(path: string): Promise<void> {
  const r = await commands.codingPlanOpenFile(path);
  if (r.status !== "ok") throw new Error(r.error.message ?? "coding_plan_open_file failed");
}
