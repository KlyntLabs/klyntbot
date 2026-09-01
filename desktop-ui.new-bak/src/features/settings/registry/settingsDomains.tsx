import { SettingsAppSection } from "@settings/components/sections/SettingsAppSection";
import { SettingsModelsSection } from "@settings/components/sections/SettingsModelsSection";
import Brain from "lucide-react/dist/esm/icons/brain";
import Monitor from "lucide-react/dist/esm/icons/monitor";
import type { ComponentType, ReactNode } from "react";

export type SettingsDomain = {
  id: string;
  label: string;
  icon?: ReactNode;
  Component: ComponentType;
};

export const settingsDomains: SettingsDomain[] = [
  {
    id: "ui",
    label: "App & UI",
    icon: <Monitor aria-hidden />,
    Component: SettingsAppSection,
  },
  {
    id: "models",
    label: "Models & Providers",
    icon: <Brain aria-hidden />,
    Component: SettingsModelsSection,
  },
];
