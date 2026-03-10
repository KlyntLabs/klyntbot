# Klyntbot Desktop UI — Restructuring Plan

> **Phiên bản**: 1.0 | **Ngày**: 2026-03-10
> **Mục tiêu**: Atomic Design, High Performance, Multi-theme, Scalable Architecture

---

## 1. TỔNG QUAN HIỆN TRẠNG

### 1.1 Thống kê codebase

| Metric | Giá trị |
|--------|---------|
| Tổng component files | ~188 |
| Tổng LOC (components) | ~29,164 |
| Components > 300 LOC | 36 |
| Components > 500 LOC | 4 |
| Barrel exports (index.ts) | 1 (chỉ vim/) |
| Hooks | 18 |
| CSS files | 6 |
| Feature domains | 10+ (chat, dashboard, finance, productivity, tasks, notes, settings, setup, tray, debug) |

### 1.2 Tech Stack hiện tại

- **React 19** + React Compiler (babel-plugin-react-compiler)
- **Tailwind CSS v4** (inline `@theme`, `@tailwindcss/vite` plugin, không có tailwind.config.js)
- **React Router v7** (hash router)
- **Tauri v2** IPC (qua `useIpc` hook)
- **TipTap** rich text editor (notes)
- **Recharts** charting
- **dnd-kit** drag-and-drop
- **Vite 6** build tool
- **Biome 2** linting/formatting

### 1.3 Các vấn đề chính

1. **Không có Atomic Design** — components flat, không phân tầng atoms/molecules/organisms
2. **Duplicate patterns** — 3+ layout implementations, 5+ card variants, chart duplicates
3. **Monolithic files** — `types.ts` (26K LOC), `useAgentStream.ts` (14K), `useFocusTimer.ts` (13K)
4. **Không barrel exports** — 188 files import trực tiếp qua relative paths
5. **Single hardcoded dark theme** — không support theme switching
6. **Hardcoded colors** — ~200 hardcoded hex/rgba values trong components
7. **`cn()` utility định nghĩa nhưng không dùng** — components dùng string trực tiếp
8. **Views quá lớn** — 7 views > 400 LOC, nhiều logic trộn lẫn UI
9. **Không data layer** — views gọi `useQuery` trực tiếp, 8+ queries/view
10. **Không shared component library** — dialog, form, list đều tự implement riêng

---

## 2. KIẾN TRÚC MỚI — TỔNG QUAN

### 2.1 Folder Structure (Feature-first + Atomic Design)

```
src/
├── app/                          # App shell, routing, providers
│   ├── App.tsx                   # Root component
│   ├── router.tsx                # Route definitions (extracted from App.tsx)
│   ├── providers/                # All React context providers
│   │   ├── ThemeProvider.tsx     # Theme switching context
│   │   ├── QueryProvider.tsx     # (nếu migrate sang react-query tương lai)
│   │   └── index.ts
│   └── layouts/                  # Route-level layouts
│       ├── AppShell.tsx
│       ├── SetupLayout.tsx
│       └── index.ts
│
├── shared/                       # Shared code (KHÔNG phụ thuộc feature nào)
│   ├── ui/                       # 🔵 ATOMS — Primitive building blocks
│   │   ├── Button.tsx
│   │   ├── Badge.tsx
│   │   ├── Checkbox.tsx
│   │   ├── Input.tsx
│   │   ├── SecretInput.tsx
│   │   ├── Toggle.tsx
│   │   ├── Progress.tsx
│   │   ├── Skeleton.tsx
│   │   ├── Spinner.tsx
│   │   ├── KlyntLogo.tsx
│   │   └── index.ts             # Barrel export
│   │
│   ├── composites/               # 🟢 MOLECULES — Atom compositions
│   │   ├── Card/
│   │   │   ├── Card.tsx          # Base Card + CardHeader + CardContent + CardFooter
│   │   │   ├── GlassCard.tsx     # Card variant với glass-card styling
│   │   │   └── index.ts
│   │   ├── Dialog/
│   │   │   ├── Dialog.tsx        # Base dialog shell (portal + backdrop + animation)
│   │   │   ├── ConfirmDialog.tsx # Confirm/cancel variant
│   │   │   └── index.ts
│   │   ├── ContextMenu/
│   │   │   ├── ContextMenu.tsx
│   │   │   └── index.ts
│   │   ├── DataTable/
│   │   │   ├── DataTable.tsx     # Generic sortable/filterable table
│   │   │   ├── TableColumn.tsx
│   │   │   └── index.ts
│   │   ├── Form/
│   │   │   ├── FormField.tsx     # Label + input + error wrapper
│   │   │   ├── FormSection.tsx   # Group of fields với heading
│   │   │   ├── InlineEditor.tsx  # Base inline edit (text, date, select)
│   │   │   └── index.ts
│   │   ├── Chart/
│   │   │   ├── DonutChart.tsx    # Unified donut (consolidate finance/Donut + productivity/BreakdownDonuts)
│   │   │   ├── ProgressRing.tsx  # Unified ring (consolidate ProductivityScoreRing + LiveScoreRing)
│   │   │   ├── BarChart.tsx      # Wrapper around recharts
│   │   │   └── index.ts
│   │   ├── DateNavigator/
│   │   │   ├── DateNavigator.tsx # Shared date nav (consolidate từ productivity + dashboard)
│   │   │   └── index.ts
│   │   ├── PageHeader/
│   │   │   ├── PageHeader.tsx    # Unified layout header (title + actions + nav)
│   │   │   └── index.ts
│   │   ├── SlidePanel/
│   │   │   ├── SlidePanel.tsx    # Reusable side panel (consolidate finance/SlidePanel)
│   │   │   └── index.ts
│   │   ├── EmptyState/
│   │   │   ├── EmptyState.tsx
│   │   │   └── index.ts
│   │   └── index.ts             # Barrel export all composites
│   │
│   ├── hooks/                    # Shared hooks (không feature-specific)
│   │   ├── useIpc.ts            # Tauri/HTTP IPC abstraction
│   │   ├── useQuery.ts          # SWR cache hook
│   │   ├── useMutation.ts       # IPC mutation wrapper
│   │   ├── useClickOutside.ts
│   │   ├── useAutoResizeTextarea.ts
│   │   ├── useSetToggle.ts
│   │   ├── useEvent.ts
│   │   ├── useWindowAutoResize.ts
│   │   ├── useTransparentBackground.ts
│   │   └── index.ts
│   │
│   ├── lib/                      # Pure utility functions
│   │   ├── cn.ts                 # clsx + twMerge (standalone file)
│   │   ├── dates.ts              # Date utilities
│   │   ├── format.ts             # formatDuration, formatTokens, formatCost, etc.
│   │   ├── errors.ts             # parseApiError, error types
│   │   ├── group-by.ts           # groupBy utility
│   │   └── index.ts
│   │
│   ├── types/                    # 🔴 SPLIT types.ts thành modules
│   │   ├── common.ts             # ApiError, Pagination, etc.
│   │   ├── chat.ts               # ChatMessage, Thread, Session types
│   │   ├── tasks.ts              # Task, Project, Objective types
│   │   ├── finance.ts            # Account, Transaction, Budget types
│   │   ├── productivity.ts       # ActivitySession, FocusSession types
│   │   ├── notes.ts              # Note, NoteFolder types
│   │   ├── config.ts             # AppConfig, Provider types
│   │   ├── agent.ts              # Agent, Pipeline, Tool types
│   │   ├── dashboard.ts          # Calendar, Event types
│   │   └── index.ts              # Re-export everything
│   │
│   └── styles/                   # Global styles
│       ├── theme.css             # CSS variables + @theme
│       ├── themes/               # 🆕 Theme variant files
│       │   ├── dark.css          # Dark theme (current default)
│       │   ├── retro.css         # Retro/CRT theme
│       │   └── _base.css         # Shared structure (border-radius, spacing, etc.)
│       ├── glass.css             # 🆕 Extracted glass-* utilities
│       ├── editor.css
│       ├── prose.css
│       ├── fonts.css
│       ├── index.css
│       └── tailwind.css
│
├── features/                     # 🟠 ORGANISMS + PAGES — Feature modules
│   ├── chat/
│   │   ├── components/           # Chat-specific components
│   │   │   ├── ChatInput.tsx
│   │   │   ├── MessageList.tsx
│   │   │   ├── SegmentedMessage.tsx
│   │   │   ├── MarkdownContent.tsx
│   │   │   ├── ThreadList.tsx
│   │   │   ├── ThreadButton.tsx
│   │   │   ├── ThreadContextMenu.tsx
│   │   │   ├── CollapsedInteraction.tsx
│   │   │   ├── InteractionCard.tsx
│   │   │   ├── GroupHeader.tsx
│   │   │   ├── PlanProgress.tsx
│   │   │   ├── CoachingNudge.tsx
│   │   │   ├── TokenBadge.tsx
│   │   │   ├── TransparencyPanel.tsx
│   │   │   ├── TransparencyToggle.tsx
│   │   │   └── SidebarChat.tsx
│   │   ├── hooks/                # Chat-specific hooks
│   │   │   ├── useChatSession.ts
│   │   │   ├── useAgentStream.ts
│   │   │   ├── useGroups.ts
│   │   │   └── useCoachingNudge.ts
│   │   ├── pages/
│   │   │   └── ChatPage.tsx      # Route-level page
│   │   ├── types.ts              # (optional: re-export from shared/types/chat)
│   │   └── index.ts              # Public API barrel
│   │
│   ├── dashboard/
│   │   ├── components/
│   │   │   ├── DashboardLayout.tsx
│   │   │   ├── SummaryPanel.tsx
│   │   │   ├── DayColumnsView/   # Split large component
│   │   │   │   ├── DayColumnsView.tsx
│   │   │   │   ├── TimeGrid.tsx
│   │   │   │   └── ActivityBlock.tsx
│   │   │   ├── CalendarTrack.tsx
│   │   │   ├── ActivityTrack.tsx
│   │   │   ├── ProductivityStrip.tsx
│   │   │   ├── CalendarSync.tsx
│   │   │   └── layers.ts
│   │   ├── pages/
│   │   │   ├── DayCalendarPage.tsx
│   │   │   ├── WeekCalendarPage.tsx
│   │   │   ├── MonthCalendarPage.tsx
│   │   │   └── YearHeatmapPage.tsx
│   │   └── index.ts
│   │
│   ├── tasks/
│   │   ├── components/
│   │   │   ├── TaskTable/        # Split large component
│   │   │   │   ├── TaskTable.tsx
│   │   │   │   ├── TaskRow.tsx
│   │   │   │   ├── ColumnRenderer.tsx
│   │   │   │   ├── CustomColumnCell.tsx
│   │   │   │   ├── CustomColumnsHeader.tsx
│   │   │   │   └── TaskTableContext.tsx
│   │   │   ├── AddSubtaskRow.tsx
│   │   │   ├── KanbanBoard.tsx
│   │   │   ├── ProjectHeader.tsx
│   │   │   ├── SubtaskProgress.tsx
│   │   │   ├── Toolbar.tsx
│   │   │   ├── WorkflowPicker.tsx
│   │   │   └── editors/
│   │   │       ├── InlineDatePicker.tsx
│   │   │       ├── InlineSelect.tsx
│   │   │       ├── InlineTextEditor.tsx
│   │   │       ├── InlineTagsEditor.tsx
│   │   │       └── MiniCalendar.tsx
│   │   ├── hooks/
│   │   │   ├── useSubtasks.ts
│   │   │   ├── useCustomColumns.ts
│   │   │   └── useWorkflows.ts
│   │   ├── pages/
│   │   │   ├── TasksPage.tsx     # (hiện tại là MainApp)
│   │   │   ├── TaskDetailPage.tsx
│   │   │   ├── ProjectDetailPage.tsx
│   │   │   └── ObjectiveDetailPage.tsx
│   │   └── index.ts
│   │
│   ├── finance/
│   │   ├── components/
│   │   │   ├── FinanceLayout.tsx
│   │   │   ├── FormModal.tsx
│   │   │   └── FinanceSkeleton.tsx
│   │   ├── hooks/
│   │   │   └── useFinanceData.ts  # 🆕 Consolidate 8+ useQuery calls
│   │   ├── lib/
│   │   │   └── finance.ts         # Move from shared/lib/finance.ts
│   │   ├── pages/
│   │   │   ├── FinanceOverviewPage.tsx
│   │   │   ├── AccountsPage.tsx
│   │   │   ├── TransactionsPage.tsx
│   │   │   ├── BudgetsPage.tsx
│   │   │   ├── InvestmentsPage.tsx
│   │   │   ├── GoalsPage.tsx
│   │   │   └── LiabilitiesPage.tsx
│   │   └── index.ts
│   │
│   ├── notes/
│   │   ├── components/
│   │   │   ├── NoteEditor.tsx
│   │   │   ├── NoteList.tsx
│   │   │   ├── NoteCard.tsx
│   │   │   ├── NoteSearchBar.tsx
│   │   │   ├── NoteTags.tsx
│   │   │   ├── LinkedNotes.tsx
│   │   │   ├── NoteVersionHistory.tsx
│   │   │   ├── FileTree.tsx
│   │   │   ├── GraphView.tsx
│   │   │   └── editor/           # Keep as-is (well-structured)
│   │   │       ├── EditorCore.tsx
│   │   │       ├── EditorToolbar.tsx
│   │   │       ├── vim/          # Keep intact
│   │   │       └── ...
│   │   ├── pages/
│   │   │   └── NotesPage.tsx
│   │   └── index.ts
│   │
│   ├── productivity/
│   │   ├── components/
│   │   │   ├── ... (keep existing but consolidate charts to shared)
│   │   ├── hooks/
│   │   │   ├── useFocusTimer.ts
│   │   │   └── usePageContext.ts
│   │   ├── lib/
│   │   │   └── constants.ts      # APP_COLORS, icons (from shared.tsx)
│   │   ├── pages/
│   │   │   ├── DayPage.tsx
│   │   │   ├── WeekPage.tsx
│   │   │   ├── MonthPage.tsx
│   │   │   └── CategoriesPage.tsx
│   │   └── index.ts
│   │
│   ├── settings/
│   │   ├── components/
│   │   │   ├── SettingsLayout.tsx
│   │   │   ├── PermissionsCard.tsx
│   │   │   └── mcp/
│   │   ├── pages/
│   │   │   ├── GeneralSettings.tsx
│   │   │   ├── ConfigurationSettings.tsx
│   │   │   ├── PersonalizationSettings.tsx
│   │   │   ├── McpServersSettings.tsx
│   │   │   ├── GitSettings.tsx
│   │   │   ├── EnvironmentsSettings.tsx
│   │   │   └── ArchivedSettings.tsx
│   │   └── index.ts
│   │
│   ├── setup/
│   │   ├── components/
│   │   │   ├── SetupProgress.tsx
│   │   │   └── finance/          # Setup-specific finance forms
│   │   ├── hooks/
│   │   │   └── useSetupNavigation.ts
│   │   ├── pages/
│   │   │   ├── WelcomeStep.tsx
│   │   │   ├── ProviderStep.tsx
│   │   │   └── ...
│   │   └── index.ts
│   │
│   ├── tray/
│   │   ├── components/
│   │   │   └── FocusControl/     # Split 789-line component
│   │   │       ├── FocusControl.tsx
│   │   │       ├── FocusSession.tsx
│   │   │       ├── SessionList.tsx
│   │   │       └── SessionStats.tsx
│   │   ├── pages/
│   │   │   ├── SystemTrayPage.tsx
│   │   │   └── LauncherPage.tsx
│   │   └── index.ts
│   │
│   ├── distraction/
│   │   ├── components/
│   │   │   └── DistractionOverlay.tsx
│   │   └── index.ts
│   │
│   └── debug/
│       ├── components/
│       │   └── tabs/
│       ├── pages/
│       │   └── DebugDashboardPage.tsx
│       └── index.ts
│
└── main.tsx                      # Entry point
```

### 2.2 Layer Dependencies (Strict upward flow)

```
Layer 0: shared/types/         ← Pure types, zero dependencies
Layer 1: shared/lib/           ← Pure functions, depends on types only
Layer 2: shared/hooks/         ← React hooks, depends on lib + types
Layer 3: shared/ui/            ← Atom components, depends on hooks + lib
Layer 4: shared/composites/    ← Molecule components, depends on ui + hooks
Layer 5: features/*/           ← Feature modules, depends on shared/*
Layer 6: app/                  ← App shell, depends on features + shared
```

**Rule**: Features KHÔNG ĐƯỢC import lẫn nhau. Cross-feature communication qua shared hooks hoặc events.

---

## 3. MULTI-THEME SYSTEM

### 3.1 Architecture

```
Theme switching dựa trên CSS custom properties + data attribute:

<html data-theme="dark">     ← Default
<html data-theme="retro">    ← Retro CRT theme
```

### 3.2 Theme file structure

**`shared/styles/themes/_base.css`** — Shared structure tokens:
```css
:root {
  /* Spacing */
  --space-xs: 4px;
  --space-sm: 8px;
  --space-md: 16px;
  --space-lg: 24px;
  --space-xl: 32px;

  /* Border radius */
  --radius-sm: 6px;
  --radius-md: 10px;
  --radius-lg: 14px;
  --radius-xl: 18px;
  --radius-full: 9999px;

  /* Typography */
  --font-sans: "SF Pro Display", system-ui, sans-serif;
  --font-mono: "SF Mono", "Fira Code", monospace;

  /* Animation */
  --duration-fast: 100ms;
  --duration-normal: 200ms;
  --duration-slow: 300ms;
  --ease-out: cubic-bezier(0.16, 1, 0.3, 1);

  /* Glass material (shared structure) */
  --glass-padding: 6px;
  --glass-radius-inner: calc(var(--radius-xl) - var(--glass-padding));
}
```

**`shared/styles/themes/dark.css`** — Current Tahoe dark theme (default):
```css
[data-theme="dark"], :root {
  --background: #000000;
  --surface-lowest: rgba(255, 255, 255, 0.025);
  --surface-low: rgba(255, 255, 255, 0.04);
  --surface-base: rgba(255, 255, 255, 0.06);
  /* ... existing dark theme variables ... */
  --text-primary: #f0f2f5;
  --brand: #f97316;
  --glass-border: rgba(255, 255, 255, 0.12);
  /* ... */
}
```

**`shared/styles/themes/retro.css`** — Retro CRT theme:
```css
[data-theme="retro"] {
  /* Background — Deep dark green CRT phosphor */
  --background: #0a0f0a;

  /* Surface hierarchy — Green-tinted glass */
  --surface-lowest: rgba(0, 255, 65, 0.02);
  --surface-low: rgba(0, 255, 65, 0.04);
  --surface-base: rgba(0, 255, 65, 0.06);
  --surface-raised: rgba(0, 255, 65, 0.09);
  --surface-highest: rgba(0, 255, 65, 0.14);

  /* Text — Classic green phosphor */
  --text-primary: #33ff66;
  --text-secondary: #29cc52;
  --text-muted: #1a8035;
  --text-dim: #0f4d20;

  /* Brand — Amber accent */
  --brand: #ffb000;
  --brand-hover: #ffc940;
  --brand-glow: rgba(255, 176, 0, 0.3);

  /* Semantic (retro-tinted) */
  --success: #33ff66;
  --destructive: #ff3333;
  --info: #33ccff;
  --warning: #ffcc00;

  /* Glass — Green-tinted translucency */
  --surface-glass: rgba(0, 255, 65, 0.06);
  --surface-glass-sidebar: rgba(0, 255, 65, 0.04);
  --surface-glass-subtle: rgba(0, 255, 65, 0.05);
  --surface-glass-elevated: rgba(0, 255, 65, 0.12);
  --glass-border: rgba(0, 255, 65, 0.15);

  /* Retro-specific tokens */
  --retro-scanline: rgba(0, 0, 0, 0.08);
  --retro-glow: 0 0 10px rgba(0, 255, 65, 0.3);
  --retro-text-shadow: 0 0 8px rgba(51, 255, 102, 0.5);

  /* Typography override — Monospace for retro feel */
  --font-sans: "IBM Plex Mono", "SF Mono", "Fira Code", monospace;
}
```

### 3.3 Retro CRT Effects (CSS-only, performance-safe)

```css
/* Scanline overlay — applied to body::after */
[data-theme="retro"] body::after {
  content: "";
  position: fixed;
  inset: 0;
  pointer-events: none;
  z-index: 9999;
  background: repeating-linear-gradient(
    0deg,
    var(--retro-scanline) 0px,
    var(--retro-scanline) 1px,
    transparent 1px,
    transparent 3px
  );
}

/* Text glow effect */
[data-theme="retro"] * {
  text-shadow: var(--retro-text-shadow);
}

/* CRT vignette */
[data-theme="retro"] body::before {
  content: "";
  position: fixed;
  inset: 0;
  pointer-events: none;
  z-index: 9998;
  background: radial-gradient(
    ellipse at center,
    transparent 60%,
    rgba(0, 0, 0, 0.4) 100%
  );
}
```

### 3.4 ThemeProvider Implementation

```tsx
// shared/providers/ThemeProvider.tsx
const THEMES = ["dark", "retro"] as const;
type Theme = typeof THEMES[number];

interface ThemeContext {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

const ThemeCtx = createContext<ThemeContext>({
  theme: "dark",
  setTheme: () => {},
});

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setTheme] = useState<Theme>(() => {
    // Persist to config via IPC, fallback to "dark"
    return (localStorage.getItem("klynt-theme") as Theme) || "dark";
  });

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("klynt-theme", theme);
  }, [theme]);

  return (
    <ThemeCtx.Provider value={{ theme, setTheme }}>
      {children}
    </ThemeCtx.Provider>
  );
}

export const useTheme = () => useContext(ThemeCtx);
```

---

## 4. SHARED COMPONENT LIBRARY (Atomic Design)

### 4.1 Atoms (`shared/ui/`)

Mỗi atom nhỏ gọn (< 80 LOC), nhận props cơ bản, dùng `cn()` cho className merging:

```tsx
// Button.tsx
interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "destructive";
  size?: "sm" | "md" | "lg";
}

export function Button({ variant = "secondary", size = "md", className, ...props }: ButtonProps) {
  return (
    <button
      className={cn(
        "inline-flex items-center justify-center rounded-lg font-medium transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50",
        variants[variant],
        sizes[size],
        className
      )}
      {...props}
    />
  );
}
```

**Danh sách atoms cần tạo/migrate:**
- `Button` (mới — hiện tại mỗi component tự render button)
- `Badge` (migrate từ ui/Badge.tsx)
- `Checkbox` (migrate từ ui/Checkbox.tsx)
- `Input` (mới — extract common input pattern)
- `SecretInput` (migrate từ shared/SecretInput.tsx)
- `Toggle` (migrate từ shared/Toggle.tsx)
- `Progress` (migrate từ ui/Progress.tsx)
- `Skeleton` (mới — extract từ TaskTableSkeleton, FinanceSkeleton)
- `Spinner` (mới)
- `KlyntLogo` (migrate từ ui/KlyntLogo.tsx)
- `Tooltip` (mới)
- `Avatar` (mới nếu cần)

### 4.2 Molecules (`shared/composites/`)

**Card system** (consolidate 5+ variants):

```tsx
// Card/Card.tsx — Compound component pattern
interface CardProps {
  variant?: "glass" | "surface" | "outline";
  padding?: "sm" | "md" | "lg";
  interactive?: boolean;  // hover effect
  className?: string;
  children: ReactNode;
}

function Card({ variant = "glass", padding = "md", interactive, className, children }: CardProps) {
  return (
    <div className={cn(
      "rounded-xl border transition-colors",
      cardVariants[variant],
      cardPadding[padding],
      interactive && "cursor-pointer hover:border-white/20",
      className
    )}>
      {children}
    </div>
  );
}

function CardHeader({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={cn("flex items-center justify-between", className)}>{children}</div>;
}

function CardContent({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={cn("mt-3", className)}>{children}</div>;
}

// Attach sub-components
Card.Header = CardHeader;
Card.Content = CardContent;
export { Card };
```

**Dialog system** (unified):

```tsx
// Dialog/Dialog.tsx — Portal-based, keyboard-accessible
interface DialogProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  size?: "sm" | "md" | "lg";
  children: ReactNode;
}

export function Dialog({ open, onClose, title, size = "md", children }: DialogProps) {
  // Portal + focus trap + Escape key + backdrop click
  // Reuse glass-panel styling
}
```

**PageHeader** (consolidate 3 layout headers):

```tsx
// PageHeader/PageHeader.tsx
interface PageHeaderProps {
  title?: ReactNode;
  nav?: ReactNode;       // Date navigator, tabs, etc.
  actions?: ReactNode;   // Buttons, toggles
  className?: string;
}

export function PageHeader({ title, nav, actions, className }: PageHeaderProps) {
  return (
    <header className={cn("flex items-center gap-3 px-5 py-3 glass-toolbar", className)}>
      {title && <h1 className="text-sm font-semibold text-primary">{title}</h1>}
      {nav && <nav className="flex items-center gap-2">{nav}</nav>}
      {actions && <div className="ml-auto flex items-center gap-2">{actions}</div>}
    </header>
  );
}
```

### 4.3 Barrel Exports

Mỗi module có `index.ts` export public API:

```ts
// shared/ui/index.ts
export { Button, type ButtonProps } from "./Button";
export { Badge, type BadgeProps } from "./Badge";
export { Checkbox } from "./Checkbox";
export { Input, type InputProps } from "./Input";
// ...

// shared/composites/index.ts
export { Card } from "./Card";
export { Dialog, ConfirmDialog } from "./Dialog";
export { DataTable, type Column } from "./DataTable";
export { PageHeader } from "./PageHeader";
// ...
```

Import pattern mới:
```tsx
// Trước: 5 relative imports
import { Badge } from "../../components/ui/Badge";
import { Card, CardHeader } from "../../components/finance/Card";
import { Toggle } from "../../components/shared/Toggle";

// Sau: 2 clean imports
import { Badge, Toggle } from "@/shared/ui";
import { Card } from "@/shared/composites";
```

---

## 5. STATE MANAGEMENT NÂNG CẤP

### 5.1 Data Layer Pattern

Thay vì 8+ `useQuery` calls rải rác trong views, tạo **feature-level data hooks**:

```tsx
// features/finance/hooks/useFinanceData.ts
export function useFinanceOverview() {
  const accounts = useQuery<Account[]>("finance_accounts", undefined, []);
  const transactions = useQuery<Transaction[]>("finance_transactions", { limit: 10 }, []);
  const budgets = useQuery<BudgetUsage[]>("finance_budget_usage", undefined, []);
  const goals = useQuery<FinanceGoal[]>("finance_goals", undefined, []);
  const liabilities = useQuery<Liability[]>("finance_liabilities", undefined, []);
  const investments = useQuery<Investment[]>("finance_investments", undefined, []);

  const loading = accounts.loading || transactions.loading || budgets.loading;
  const netWorth = useMemo(() => computeNetWorth(accounts.data, liabilities.data), [accounts.data, liabilities.data]);

  return {
    accounts: accounts.data,
    transactions: transactions.data,
    budgets: budgets.data,
    goals: goals.data,
    netWorth,
    loading,
    refetch: () => {
      accounts.refetch();
      transactions.refetch();
      budgets.refetch();
    },
  };
}
```

### 5.2 Vite Path Aliases

```ts
// vite.config.ts — thêm aliases
resolve: {
  alias: {
    "@": path.resolve(__dirname, "./src"),
    "@shared": path.resolve(__dirname, "./src/shared"),
    "@features": path.resolve(__dirname, "./src/features"),
    "@app": path.resolve(__dirname, "./src/app"),
  },
},
```

```json
// tsconfig.json — thêm paths
{
  "compilerOptions": {
    "paths": {
      "@/*": ["./src/*"],
      "@shared/*": ["./src/shared/*"],
      "@features/*": ["./src/features/*"],
      "@app/*": ["./src/app/*"]
    }
  }
}
```

---

## 6. PERFORMANCE OPTIMIZATIONS

### 6.1 React Compiler (đã có)

Project đã dùng `babel-plugin-react-compiler` — tốt. Đây là automatic memoization, không cần manual `useMemo`/`useCallback` cho hầu hết cases.

### 6.2 Code Splitting Strategy

```
Hiện tại: ✅ Tất cả routes đã lazy() — good
Nâng cấp cần thiết:
- Feature-level code splitting (mỗi feature/ folder là 1 chunk)
- Heavy libs (recharts, d3-force, tiptap) lazy-load on demand
```

```ts
// vite.config.ts — manual chunks
build: {
  rollupOptions: {
    output: {
      manualChunks: {
        "vendor-react": ["react", "react-dom", "react-router"],
        "vendor-editor": ["@tiptap/react", "@tiptap/starter-kit", "@tiptap/pm"],
        "vendor-charts": ["recharts", "d3-force"],
      },
    },
  },
},
```

### 6.3 CSS Performance

- Glass effects (`backdrop-filter`) đã có `.resizing` disable — good
- Retro scanline effect dùng `pointer-events: none` + `position: fixed` — rasterized 1 lần
- `will-change: transform` cho animated elements

---

## 7. MIGRATION STRATEGY (Incremental)

### Phase 1: Foundation (1-2 ngày)
1. Setup path aliases (`@/`, `@shared/`, `@features/`)
2. Tạo folder structure mới (song song với cũ)
3. Tạo `shared/types/` — split `types.ts` thành modules
4. Tạo `shared/lib/` — move + reorganize utils
5. Tạo theme infrastructure (`ThemeProvider`, `data-theme`, CSS split)

### Phase 2: Shared UI Library (2-3 ngày)
1. Build `shared/ui/` atoms (Button, Input, Badge, etc.)
2. Build `shared/composites/` molecules (Card, Dialog, PageHeader, Chart)
3. Add barrel exports
4. Bắt đầu migrate features dùng shared components

### Phase 3: Feature Migration (3-5 ngày, per feature)
1. Migrate từng feature module sang `features/` structure
2. Split large components (FocusControl, DayColumnsView, Finance views)
3. Create feature-level data hooks
4. Update imports to use `@/` aliases
5. Delete old `components/` files sau khi migrate xong

### Phase 4: Retro Theme (1-2 ngày)
1. Implement retro.css variables
2. Add CRT effects (scanlines, vignette, glow)
3. Add theme switcher UI trong Settings > Personalization
4. Test all components under retro theme
5. Fix hardcoded colors

### Phase 5: Cleanup & Polish (1-2 ngày)
1. Remove old folder structure
2. Update all remaining hardcoded colors
3. Enforce `cn()` usage everywhere via lint rule
4. Audit bundle size
5. Performance testing

**Tổng thời gian ước tính: 8-14 ngày**

---

## 8. NAMING CONVENTIONS

| Item | Convention | Example |
|------|-----------|---------|
| Component file | PascalCase.tsx | `PageHeader.tsx` |
| Hook file | camelCase.ts | `useFinanceData.ts` |
| Utility file | kebab-case.ts | `group-by.ts` |
| Type file | kebab-case.ts | `common.ts` |
| CSS file | kebab-case.css | `retro.css` |
| Barrel export | index.ts | `index.ts` |
| Feature folder | kebab-case | `features/finance/` |
| Component folder | PascalCase | `composites/Card/` |
| Test file | *.test.ts(x) | `Card.test.tsx` |

---

## 9. ESLINT/BIOME RULES (Đề xuất thêm)

```json
// biome.json — thêm import restriction rules
{
  "linter": {
    "rules": {
      "correctness": {
        "noUnusedImports": "error"
      },
      "style": {
        "noNamespaceImport": "warn"
      }
    }
  }
}
```

**Custom lint rules cần enforcement:**
- No cross-feature imports (features/A không import features/B)
- No hardcoded hex colors trong `.tsx` files (trừ `constants.ts`)
- Prefer `cn()` over string concatenation cho className

---

## 10. KẾT LUẬN

Restructuring plan này transform codebase từ flat component architecture sang **feature-based Atomic Design** với:

1. **Scalability**: Feature modules độc lập, thêm feature mới không ảnh hưởng cũ
2. **Reusability**: Shared UI library (atoms + molecules) dùng chung 100%
3. **Performance**: Code splitting per feature, React Compiler, optimized CSS
4. **Maintainability**: Barrel exports, path aliases, consistent naming
5. **Multi-theme**: CSS variable-based theme switching (dark + retro)
6. **Developer Experience**: Clean imports, type-safe, lint-enforced boundaries

**Breaking changes được chấp nhận** — mọi import paths sẽ thay đổi, nhưng không có logic changes. Migration có thể incremental, feature-by-feature.
