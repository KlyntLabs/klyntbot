/* ═══════════════════════════════════════════════════════════════════════════
   Klynt Tailwind Design System
   ═══════════════════════════════════════════════════════════════════════════
   CVA-based primitives that map to the existing CSS custom property tokens.
   Import from here when building new UI or refactoring legacy BEM components.
   ══════════════════════════════════════════════════════════════════════════ */

// ── Layout ──
export { Box } from "./box";
export type { BoxProps } from "./box";

export { Stack, HStack, VStack } from "./stack";
export type { StackProps } from "./stack";

export { Grid, Container } from "./grid";
export type { GridProps, ContainerProps } from "./grid";

// ── Typography ──
export { Text, Heading } from "./text";
export type { TextProps, HeadingProps } from "./text";

// ── Surfaces ──
export { Surface, Divider } from "./surface";
export type { SurfaceProps, DividerProps } from "./surface";

// ── Feedback ──
export { Skeleton } from "./skeleton";
export type { SkeletonProps } from "./skeleton";

export { Spinner } from "./spinner";
export type { SpinnerProps } from "./spinner";

// ── Forms ──
export { Label } from "./label";
export type { LabelProps } from "./label";

export { Textarea } from "./textarea";
export type { TextareaProps } from "./textarea";

export { Switch } from "./switch";
export type { SwitchProps } from "./switch";

// ── Data ──
export { Avatar } from "./avatar";
export type { AvatarProps } from "./avatar";

export { Chip } from "./chip";
export type { ChipProps } from "./chip";

// ── Existing primitives ──
export { Button } from "./button";
export type { ButtonProps } from "./button";

export { Badge } from "./badge";
export type { BadgeProps } from "./badge";

export { Input, SearchField } from "./input";
export type { InputProps, SearchFieldProps } from "./input";

export { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from "./card";

export {
  PanelFrame,
  PanelHeader,
  PanelMeta,
  PanelNavList,
  PanelNavItem,
  PanelSearchField,
} from "./panel";
export type { PanelNavItemProps, PanelSearchFieldProps } from "./panel";
