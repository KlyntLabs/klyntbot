// Productivity Feature - Public API

// Pages
export { ProductivityDayPage } from "./pages/DayPage";
export { ProductivityWeekPage } from "./pages/WeekPage";
export { ProductivityMonthPage } from "./pages/MonthPage";
export { CategoriesPage } from "./pages/CategoriesPage";

// Components
export { ProductivityLayout } from "./components/ProductivityLayout";
export { DayView } from "./components/DayView";
export { WeekView } from "./components/WeekView";
export { MonthView } from "./components/MonthView";
export { FocusSessionsList } from "./components/FocusSessionsList";
export { PomodoroTimer } from "./components/PomodoroTimer";
export { CategoryList } from "./components/CategoryList";
export { CategoryEditor } from "./components/CategoryEditor";
export { TrackedAppsList } from "./components/TrackedAppsList";
export { DateNavigator } from "./components/DateNavigator";

// Hooks
export { useFocusTimer, FOCUS_PRESETS } from "./hooks/useFocusTimer";
export type { FocusSettings, FocusPreset } from "./hooks/useFocusTimer";
export { usePageContext } from "./hooks/usePageContext";

// Constants & utilities
export {
  APP_COLORS,
  APP_ICONS,
  getAppColor,
  AppIcon,
  CATEGORY_TYPE_GROUPS,
  TYPE_BADGE_COLORS,
  DEFAULT_CATEGORY_COLOR,
  getCategoryColor,
  getCategoryTypeColor,
  resolveActivityColor,
  resolveCategoryLabel,
  scoreColor,
  buildBreakdownSegments,
  PRODUCTIVITY_LEGEND,
  ChartTooltip,
} from "./lib/constants";
