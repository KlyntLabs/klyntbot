import {
  Code,
  Database,
  Folder,
  Globe,
  Layers,
  Layout,
  type LucideProps,
  Palette,
  Shield,
  Terminal,
  Zap,
} from "lucide-react";
import type React from "react";

export interface Project {
  id: string;
  name: string;
  icon: React.FC<LucideProps>;
}

export const projects: Project[] = [
  { id: "1", name: "Core Platform", icon: Code },
  { id: "2", name: "Design System", icon: Palette },
  { id: "3", name: "Auth Service", icon: Shield },
  { id: "4", name: "Dashboard", icon: Layout },
  { id: "5", name: "API Gateway", icon: Globe },
  { id: "6", name: "Search Engine", icon: Zap },
  { id: "7", name: "Data Pipeline", icon: Database },
  { id: "8", name: "Mobile App", icon: Layers },
  { id: "9", name: "CLI Tools", icon: Terminal },
  { id: "10", name: "Documentation", icon: Folder },
];
