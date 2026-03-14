import { createContext, useContext } from "react";

interface TasksContextValue {
  refetch: () => void;
}

const TasksContext = createContext<TasksContextValue>({ refetch: () => {} });

export const TasksProvider = TasksContext.Provider;

/** Call after any mutation to refresh the task list. */
export function useRefetchTasks() {
  return useContext(TasksContext).refetch;
}
