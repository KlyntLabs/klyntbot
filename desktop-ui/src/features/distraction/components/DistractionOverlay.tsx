import { useRef, useState } from "react";
import { useEvent } from "@/hooks/useEvent";
import { useTransparentBackground } from "@/hooks/window/useTransparentBackground";
import { useWindowAutoResize } from "@/hooks/window/useWindowAutoResize";
import { getCurrentWindow, ipc, isTauri } from "@/utils/tauri-bridge";

import "../distraction.css";

interface InterventionPayload {
	appName: string;
	windowTitle: string | null;
	sessionId: string;
	needsLlm: boolean;
	heuristicVerdict: string;
}

interface VerdictPayload {
	classification: string;
	displayText: string;
}

export function DistractionOverlay() {
	const [intervention, setIntervention] = useState<InterventionPayload | null>(
		null,
	);
	const [verdict, setVerdict] = useState<VerdictPayload | null>(null);
	const [loading, setLoading] = useState(false);
	const contentRef = useRef<HTMLDivElement>(null);
	const bodyRef = useRef<HTMLDivElement>(null);

	useTransparentBackground();
	useWindowAutoResize(bodyRef, { width: 340, minHeight: 0, maxHeight: 300 });

	useEvent<InterventionPayload>("distraction:intervention", (payload) => {
		setIntervention(payload);
		setVerdict(null);
		if (payload.needsLlm) setLoading(true);
	});

	useEvent<VerdictPayload>("distraction:verdict", (payload) => {
		setVerdict(payload);
		setLoading(false);
	});

	const titleExcerpt =
		intervention?.windowTitle && intervention.windowTitle.length > 50
			? `${intervention.windowTitle.slice(0, 50)}…`
			: (intervention?.windowTitle ?? null);

	const hideWindow = async () => {
		setIntervention(null);
		setVerdict(null);
		setLoading(false);
		if (!isTauri()) return;
		try {
			await getCurrentWindow().hide();
		} catch {
			// dev/browser mode — just clear state
		}
	};

	const pattern =
		intervention?.windowTitle?.toLowerCase() ??
		intervention?.appName.toLowerCase();

	const handleDismiss = async () => {
		if (!intervention) return;
		await ipc("distraction_dismiss", { appName: intervention.appName }).catch(
			(e) => console.error("Failed to dismiss distraction:", e),
		);
		await hideWindow();
	};

	const handleAllowTemp = async () => {
		if (!pattern) return;
		await ipc("distraction_allow_temp", { pattern }).catch((e) =>
			console.error("Failed to allow temp:", e),
		);
		await hideWindow();
	};

	const handleAllowSession = async () => {
		if (!intervention) return;
		await ipc("distraction_allow_session", {
			appName: intervention.appName,
			windowTitle: intervention.windowTitle,
			classification: verdict?.classification ?? "work_research",
		}).catch((e) => console.error("Failed to allow session:", e));
		await hideWindow();
	};

	const isPositiveVerdict =
		verdict?.classification === "educational" ||
		verdict?.classification === "work_research";

	return (
		<div className="dx-root">
			<div ref={contentRef} className="dx-shell">
				<div ref={bodyRef} className="dx-body">
					{intervention && (
						<>
							<div className="dx-header">
								<div className="dx-status">
									<span className="dx-status-dot" />
									<span className="dx-status-label">Focus active</span>
								</div>
								{(loading || verdict) && (
									<div className="dx-verdict">
										{loading && (
											<>
												<div className="dx-dots">
													<span style={{ animationDelay: "0ms" }} />
													<span style={{ animationDelay: "150ms" }} />
													<span style={{ animationDelay: "300ms" }} />
												</div>
												<span>Analyzing…</span>
											</>
										)}
										{verdict && !loading && (
											<span
												className={
													isPositiveVerdict ? "is-positive" : "is-negative"
												}
											>
												{verdict.displayText}
											</span>
										)}
									</div>
								)}
							</div>

							<div className="dx-divider" />

							<div className="dx-info">
								<div className="dx-app-name">{intervention.appName}</div>
								{titleExcerpt && (
									<div className="dx-window-title">{titleExcerpt}</div>
								)}
							</div>

							<div className="dx-actions">
								<button
									type="button"
									onClick={handleDismiss}
									className="dx-btn is-primary"
								>
									Back to work
								</button>
								<button
									type="button"
									onClick={handleAllowTemp}
									className="dx-btn"
								>
									5 min break
								</button>
								<button
									type="button"
									onClick={handleAllowSession}
									className="dx-btn"
								>
									It's work
								</button>
							</div>
						</>
					)}
				</div>
			</div>
		</div>
	);
}
