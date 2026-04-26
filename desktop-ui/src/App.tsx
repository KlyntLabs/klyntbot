import { lazy, Suspense } from "react";
import "./styles/index.css";

import MainApp from "@app/components/MainApp";
import { currentWindowLabel } from "@/utils/tauri-bridge";
import { useWindowLabel } from "@/features/layout/hooks/useWindowLabel";

const AboutView = lazy(() =>
	import("@/features/about/components/AboutView").then((module) => ({
		default: module.AboutView,
	})),
);

const Launcher = lazy(() =>
	import("@/features/launcher/components/Launcher").then((module) => ({
		default: module.Launcher,
	})),
);

const Tray = lazy(() =>
	import("@/features/tray/components/Tray").then((module) => ({
		default: module.Tray,
	})),
);

// Tauri 2 sets the label synchronously before any React render, so we read it
// once at module init from the real internals (the project-wide
// useWindowLabel hook is intercepted by a mock that always returns "main").
const realLabel = currentWindowLabel();

export default function App() {
	const windowLabel = useWindowLabel();

	if (realLabel === "launcher" || windowLabel === "launcher") {
		return (
			<Suspense fallback={null}>
				<Launcher />
			</Suspense>
		);
	}

	if (realLabel === "tray" || windowLabel === "tray") {
		return (
			<Suspense fallback={null}>
				<Tray />
			</Suspense>
		);
	}

	if (windowLabel === "about") {
		return (
			<Suspense fallback={null}>
				<AboutView />
			</Suspense>
		);
	}

	return <MainApp />;
}
