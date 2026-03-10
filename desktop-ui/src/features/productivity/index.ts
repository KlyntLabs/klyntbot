// Productivity Feature - Public API

export { CategoryEditor } from "./components/CategoryEditor";
export { CategoryList } from "./components/CategoryList";
export { DateNavigator } from "./components/DateNavigator";
export { DayView } from "./components/DayView";
export { FocusSessionsList } from "./components/FocusSessionsList";
export { MonthView } from "./components/MonthView";
export { PomodoroTimer } from "./components/PomodoroTimer";
// Components
export { ProductivityLayout } from "./components/ProductivityLayout";
export { TrackedAppsList } from "./components/TrackedAppsList";
export { WeekView } from "./components/WeekView";
export type { FocusPreset, FocusSettings } from "./hooks/useFocusTimer";
// Hooks
export { FOCUS_PRESETS, useFocusTimer } from "./hooks/useFocusTimer";
export { usePageContext } from "./hooks/usePageContext";
// Constants & utilities
export {
  APP_COLORS,
  APP_ICONS,
  AppIcon,
  buildBreakdownSegments,
  CATEGORY_TYPE_GROUPS,
  ChartTooltip,
  DEFAULT_CATEGORY_COLOR,
  getAppColor,
  getCategoryColor,
  getCategoryTypeColor,
  PRODUCTIVITY_LEGEND,
  resolveActivityColor,
  resolveCategoryLabel,
  scoreColor,
  TYPE_BADGE_COLORS,
} from "./lib/constants";
export { CategoriesPage } from "./pages/CategoriesPage";
// Pages
export { ProductivityDayPage } from "./pages/DayPage";
export { ProductivityMonthPage } from "./pages/MonthPage";
export { ProductivityWeekPage } from "./pages/WeekPage";
