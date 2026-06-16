export const AppView = {
  Home: "home",
  Chat: "chat",
  Plugins: "plugins",
  Calendar: "calendar",
  Focus: "focus",
} as const;

export type AppView = (typeof AppView)[keyof typeof AppView];
