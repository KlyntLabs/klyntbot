import type { SlashNode } from "./types";

export const REGISTRY: Record<string, SlashNode> = {
  plan: {
    kind: "leaf",
    path: "agent",
    command: "plan",
    description: "Enter plan mode (writes/exec denied)",
    category: "mode",
  },
  yolo: {
    kind: "leaf",
    path: "agent",
    command: "yolo",
    description: "Bypass approvals (requires env var)",
    category: "mode",
  },
  power: {
    kind: "branch",
    command: "power",
    description: "Toggle power tool profile",
    category: "mode",
    children: {
      on: {
        kind: "leaf",
        path: "agent",
        command: "power on",
        description: "Enable power tools",
        category: "mode",
      },
      off: {
        kind: "leaf",
        path: "agent",
        command: "power off",
        description: "Disable power tools",
        category: "mode",
      },
    },
  },
  recall: {
    kind: "leaf",
    path: "agent",
    command: "recall",
    description: "Force a recall pass with this query",
    category: "recall",
    argHint: "<query>",
  },
  skills: {
    kind: "branch",
    command: "skills",
    description: "Manage skills",
    category: "skills",
    children: {
      list: {
        kind: "leaf",
        path: "direct",
        command: "skills list",
        tauriCommand: "coding_skills_list",
        description: "List installed skills",
        category: "skills",
      },
      info: {
        kind: "leaf",
        path: "direct",
        command: "skills info",
        tauriCommand: "coding_skills_info",
        description: "Show skill frontmatter",
        category: "skills",
        argHint: "<name>",
      },
      install: {
        kind: "leaf",
        path: "direct",
        command: "skills install",
        tauriCommand: "coding_skills_install",
        description: "Install a skill (path or URL)",
        category: "skills",
        argHint: "<source>",
      },
      update: {
        kind: "leaf",
        path: "direct",
        command: "skills update",
        tauriCommand: "coding_skills_update",
        description: "Re-fetch skill from origin",
        category: "skills",
        argHint: "<name>",
      },
      uninstall: {
        kind: "leaf",
        path: "direct",
        command: "skills uninstall",
        tauriCommand: "coding_skills_uninstall",
        description: "Remove a skill",
        category: "skills",
        argHint: "<name>",
      },
      toggle: {
        kind: "leaf",
        path: "direct",
        command: "skills toggle",
        tauriCommand: "coding_skills_toggle",
        description: "Enable/disable a skill",
        category: "skills",
        argHint: "<name> --on|--off",
      },
      validate: {
        kind: "leaf",
        path: "direct",
        command: "skills validate",
        tauriCommand: "coding_skills_validate",
        description: "Check SKILL.md validity",
        category: "skills",
        argHint: "<name>",
      },
      reload: {
        kind: "leaf",
        path: "direct",
        command: "skills reload",
        tauriCommand: "coding_skills_reload",
        description: "Re-walk skill discovery",
        category: "skills",
      },
    },
  },
  status: {
    kind: "leaf",
    path: "direct",
    command: "status",
    tauriCommand: "coding_status",
    description: "Show mode, profile, sandbox, cost",
    category: "status",
  },
  doctor: {
    kind: "leaf",
    path: "direct",
    command: "doctor",
    tauriCommand: "coding_doctor",
    description: "Run diagnostic checklist",
    category: "status",
  },
  sessions: {
    kind: "branch",
    command: "sessions",
    description: "Manage chat sessions",
    category: "sessions",
    children: {
      star: {
        kind: "leaf",
        path: "direct",
        command: "sessions star",
        tauriCommand: "coding_sessions_star",
        description: "Mark current thread starred",
        category: "sessions",
      },
      unstar: {
        kind: "leaf",
        path: "direct",
        command: "sessions unstar",
        tauriCommand: "coding_sessions_unstar",
        description: "Unmark current thread",
        category: "sessions",
      },
      rewind: {
        kind: "leaf",
        path: "direct",
        tauriCommand: "coding_sessions_rewind",
        command: "sessions rewind",
        description: "Restore files + delete messages back to a chosen anchor",
        argHint: "<message-id>",
        category: "sessions",
        requiresConfirmation: true,
      },
      export: {
        kind: "leaf",
        path: "direct",
        tauriCommand: "coding_sessions_export",
        command: "sessions export",
        description: "Write the thread to a file (md or json)",
        argHint: "[--format md|json]",
        category: "sessions",
      },
      fork: {
        kind: "leaf",
        path: "direct",
        tauriCommand: "coding_sessions_fork",
        command: "sessions fork",
        description: "Fork the current thread at a message",
        argHint: "[<message-id>]",
        category: "sessions",
      },
    },
  },
  permissions: {
    kind: "branch",
    command: "permissions",
    description: "Permission and approval commands",
    category: "permissions",
    children: {
      "clear-mirror": {
        kind: "leaf",
        path: "direct",
        tauriCommand: "coding_permissions_clear_mirror",
        command: "permissions clear-mirror",
        description: "Reset Mirror-learned approval cache for a tool",
        argHint: "<tool>",
        category: "permissions",
        requiresConfirmation: true,
      },
    },
  },
  "dead-ends": {
    kind: "leaf",
    path: "agent",
    command: "dead-ends",
    description: "Surface this repo's known dead-ends from Mirror",
    category: "recall",
    agentTransform: () => "[system: invoke check_dead_ends for the current repo and summarize]",
  },
  mirror: {
    kind: "leaf",
    path: "agent",
    command: "mirror",
    description: "Show recent Mirror alerts inline",
    category: "recall",
    agentTransform: () => "[system: list the agent's recent Mirror alerts]",
  },
  resume: {
    kind: "leaf",
    path: "direct",
    command: "resume",
    tauriCommand: "coding_resume",
    description: "Switch to matching thread",
    category: "sessions",
    argHint: "<prefix>",
  },
  help: {
    kind: "leaf",
    path: "direct",
    command: "help",
    tauriCommand: "coding_help",
    description: "List slash commands",
    category: "help",
    argHint: "[command]",
  },
};

function buildFlatCatalog(): Array<{
  command: string;
  description: string;
  category: string;
  argHint?: string;
}> {
  const out: Array<{ command: string; description: string; category: string; argHint?: string }> =
    [];
  function walk(node: SlashNode) {
    if (node.kind === "leaf") {
      out.push({
        command: `/${node.command}`,
        description: node.description,
        category: node.category,
        argHint: node.argHint,
      });
    } else {
      for (const child of Object.values(node.children)) walk(child);
    }
  }
  for (const node of Object.values(REGISTRY)) walk(node);
  return out;
}

export const FLAT_CATALOG = buildFlatCatalog();

/** @deprecated Use `FLAT_CATALOG` instead. */
export function flatCatalog() {
  return FLAT_CATALOG;
}
