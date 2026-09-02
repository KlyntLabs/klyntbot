import "@klyntbot/design-system/styles/index.css";
import React from "react";
import { createRoot } from "react-dom/client";
import { StatusBadge } from "./StatusBadge";

createRoot(document.getElementById("root")!).render(<StatusBadge />);
