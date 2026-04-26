import { useEffect } from "react";

// Forces html/body to transparent so the Tauri HUD material shows through.
// Without this, the new desktop-ui's default surfaces paint a dark rectangle
// behind the launcher card, leaving an empty void below.
export function useTransparentBackground() {
	useEffect(() => {
		const html = document.documentElement;
		const body = document.body;
		const prevHtmlBg = html.style.background;
		const prevBodyBg = body.style.background;
		const prevHtmlColor = html.style.backgroundColor;
		const prevBodyColor = body.style.backgroundColor;
		html.style.background = "transparent";
		body.style.background = "transparent";
		html.style.backgroundColor = "transparent";
		body.style.backgroundColor = "transparent";
		html.dataset.launcher = "true";
		return () => {
			html.style.background = prevHtmlBg;
			body.style.background = prevBodyBg;
			html.style.backgroundColor = prevHtmlColor;
			body.style.backgroundColor = prevBodyColor;
			delete html.dataset.launcher;
		};
	}, []);
}
