export type SkillSourceType = "github" | "skills_sh" | "local" | "bundled";

export interface InstalledSkill {
  name: string;
  sourceType: SkillSourceType;
  sourceRef: string;
  installedVersion: string;
  installedSha: string;
  enabled: boolean;
  isAdapted: boolean;
  bootstrappedDatabases: string[];
  installedAt: string;
  updatedAt: string;
}

export interface SkillBrowseRow {
  rank: number;
  name: string;
  sourceRef: string;
  installs?: number;
  isKlyntNative: boolean;
  isInstalled: boolean;
  isBundled: boolean;
}

export interface FileWrite {
  relativePath: string;
  contentSize: number;
}

export interface TemplatePreview {
  templateName: string;
  databaseName: string;
  fieldCount: number;
}

export interface InstallPlan {
  package: {
    name: string;
    resolvedSha: string;
    semver?: string;
    skillMdContent: string;
    klyntbotMeta?: unknown;
    templates: { name: string; manifest: unknown }[];
  };
  filesToWrite: FileWrite[];
  databasesToBootstrap: TemplatePreview[];
  warnings: string[];
}

export interface DiffLine {
  tag: "equal" | "insert" | "delete";
  text: string;
}

export interface FrontmatterChange {
  field: string;
  before?: unknown;
  after?: unknown;
}

export interface DiffResult {
  bodyLines: DiffLine[];
  frontmatterChanges: FrontmatterChange[];
  bootstrapsAdded: string[];
  bootstrapsRemoved: string[];
}

export interface AvailableVersion {
  sha: string;
  tag?: string;
  message: string;
  date: string;
}

export interface UpgradePlan {
  name: string;
  fromSha: string;
  toSha: string;
  diff: DiffResult;
  newBootstraps: TemplatePreview[];
}

export type UninstallMode = "skill_only" | "archive_databases" | "delete_databases";
