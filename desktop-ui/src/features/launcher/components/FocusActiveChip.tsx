import { formatRemaining } from "../lib/formatRemaining";
import { ipc } from "@/utils/tauri-bridge";

interface Props {
	endsAt: string;
	onDone: () => void;
}

export function FocusActiveChip({ endsAt, onDone }: Props) {
	const handleExtend = (extraMs: number) => {
		const base = Math.max(new Date(endsAt).getTime(), Date.now());
		const newEndsAt = new Date(base + extraMs).toISOString();
		ipc("focus_extend", { mode: "dnd", newEndsAt })
			.then(() => onDone())
			.catch((err) => console.error("focus_extend failed:", err));
	};

	const handleTurnOff = () => {
		ipc("focus_deactivate", { mode: "dnd" })
			.then(() => onDone())
			.catch((err) => console.error("focus_deactivate failed:", err));
	};

	return (
		<div className="lc-arg-bar">
			<span className="lc-muted-sm">
				DND on — {formatRemaining(endsAt)} left
			</span>
			<button
				type="button"
				className="lc-chip-btn"
				onClick={() => handleExtend(30 * 60_000)}
			>
				+30m
			</button>
			<button
				type="button"
				className="lc-chip-btn"
				onClick={() => handleExtend(2 * 3_600_000)}
			>
				+2h
			</button>
			<button
				type="button"
				className="lc-chip-btn is-danger"
				onClick={handleTurnOff}
			>
				Turn off
			</button>
		</div>
	);
}
