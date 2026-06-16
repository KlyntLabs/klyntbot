export const AppView = {
  Home: "home",
  Chat: "chat",
  Calendar: "calendar",
} as const;

export type AppView = (typeof AppView)[keyof typeof AppView];
