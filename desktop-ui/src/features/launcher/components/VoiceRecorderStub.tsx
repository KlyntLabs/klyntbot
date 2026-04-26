import { useEffect } from "react";

interface Props {
	onTranscriptReady: (transcript: string) => void;
	onCancel: () => void;
}

export function VoiceRecorderStub({ onCancel }: Props) {
	useEffect(() => {
		const handler = (e: KeyboardEvent) => {
			if (e.key === "Escape" || e.key === "Enter") {
				e.preventDefault();
				onCancel();
			}
		};
		window.addEventListener("keydown", handler);
		return () => window.removeEventListener("keydown", handler);
	}, [onCancel]);

	return (
		<div className="lc-voice-stub">
			<div className="lc-voice-stub-mic">🎙</div>
			<p className="lc-muted-sm">Voice recording stubbed.</p>
			<p className="lc-dim-xs">Press Esc to dismiss.</p>
		</div>
	);
}
