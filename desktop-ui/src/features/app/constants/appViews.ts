export const AppView = {
  Home: "home",
  Chat: "chat",
  Plugins: "plugins",
  Calendar: "calendar",
} as const;

export type AppView = (typeof AppView)[keyof typeof AppView];
