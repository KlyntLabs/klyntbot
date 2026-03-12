import { create } from "zustand";
import type { Issue } from "../mock-data/issues";
import { issues as mockIssues } from "../mock-data/issues";
import type { LabelInterface } from "../mock-data/labels";
import type { Priority } from "../mock-data/priorities";
import type { Status } from "../mock-data/status";
import type { User } from "../mock-data/users";
import type { FilterState } from "./filter-store";

interface IssuesState {
  issues: Issue[];

  // CRUD
  addIssue: (issue: Issue) => void;
  updateIssue: (id: string, partial: Partial<Issue>) => void;
  deleteIssue: (id: string) => void;

  // Filtering helpers
  filterByStatus: (statusId: string) => Issue[];
  filterByPriority: (priorityId: string) => Issue[];
  filterByAssignee: (userId: string | null) => Issue[];
  filterByLabel: (labelId: string) => Issue[];
  filterByProject: (projectId: string) => Issue[];
  searchIssues: (query: string) => Issue[];
  filterIssues: (filters: FilterState["filters"]) => Issue[];

  // Update helpers
  updateIssueStatus: (issueId: string, newStatus: Status) => void;
  updateIssuePriority: (issueId: string, newPriority: Priority) => void;
  updateIssueAssignee: (issueId: string, newAssignee: User | null) => void;

  // Label helpers
  addIssueLabel: (issueId: string, label: LabelInterface) => void;
  removeIssueLabel: (issueId: string, labelId: string) => void;

  // Utility
  getIssueById: (id: string) => Issue | undefined;
}

export const useIssuesStore = create<IssuesState>()((set, get) => ({
  issues: mockIssues,

  addIssue: (issue) => set((state) => ({ issues: [...state.issues, issue] })),

  updateIssue: (id, partial) =>
    set((state) => ({
      issues: state.issues.map((issue) => (issue.id === id ? { ...issue, ...partial } : issue)),
    })),

  deleteIssue: (id) =>
    set((state) => ({ issues: state.issues.filter((issue) => issue.id !== id) })),

  filterByStatus: (statusId) => get().issues.filter((issue) => issue.status.id === statusId),

  filterByPriority: (priorityId) =>
    get().issues.filter((issue) => issue.priority.id === priorityId),

  filterByAssignee: (userId) =>
    userId === null
      ? get().issues.filter((issue) => issue.assignee === null)
      : get().issues.filter((issue) => issue.assignee?.id === userId),

  filterByLabel: (labelId) =>
    get().issues.filter((issue) => issue.labels.some((l) => l.id === labelId)),

  filterByProject: (projectId) => get().issues.filter((issue) => issue.project?.id === projectId),

  searchIssues: (query) => {
    const q = query.toLowerCase().trim();
    if (!q) return get().issues;
    return get().issues.filter(
      (issue) =>
        issue.title.toLowerCase().includes(q) ||
        issue.description.toLowerCase().includes(q) ||
        issue.identifier.toLowerCase().includes(q),
    );
  },

  filterIssues: (filters) => {
    let result = get().issues;

    if (filters.status.length > 0) {
      result = result.filter((issue) => filters.status.includes(issue.status.id));
    }
    if (filters.priority.length > 0) {
      result = result.filter((issue) => filters.priority.includes(issue.priority.id));
    }
    if (filters.assignee.length > 0) {
      result = result.filter(
        (issue) => issue.assignee && filters.assignee.includes(issue.assignee.id),
      );
    }
    if (filters.labels.length > 0) {
      result = result.filter((issue) => issue.labels.some((l) => filters.labels.includes(l.id)));
    }
    if (filters.project.length > 0) {
      result = result.filter(
        (issue) => issue.project && filters.project.includes(issue.project.id),
      );
    }

    return result;
  },

  updateIssueStatus: (issueId, newStatus) =>
    set((state) => ({
      issues: state.issues.map((issue) =>
        issue.id === issueId ? { ...issue, status: newStatus } : issue,
      ),
    })),

  updateIssuePriority: (issueId, newPriority) =>
    set((state) => ({
      issues: state.issues.map((issue) =>
        issue.id === issueId ? { ...issue, priority: newPriority } : issue,
      ),
    })),

  updateIssueAssignee: (issueId, newAssignee) =>
    set((state) => ({
      issues: state.issues.map((issue) =>
        issue.id === issueId ? { ...issue, assignee: newAssignee } : issue,
      ),
    })),

  addIssueLabel: (issueId, label) =>
    set((state) => ({
      issues: state.issues.map((issue) => {
        if (issue.id !== issueId) return issue;
        if (issue.labels.some((l) => l.id === label.id)) return issue;
        return { ...issue, labels: [...issue.labels, label] };
      }),
    })),

  removeIssueLabel: (issueId, labelId) =>
    set((state) => ({
      issues: state.issues.map((issue) =>
        issue.id === issueId
          ? { ...issue, labels: issue.labels.filter((l) => l.id !== labelId) }
          : issue,
      ),
    })),

  getIssueById: (id) => get().issues.find((issue) => issue.id === id),
}));
