import { useRef, useState } from "react";
import type { ArgSpec } from "../types";

interface Props {
	specs: ArgSpec[];
	onSubmit: (args: Record<string, string>) => void;
	onCancel: () => void;
}

export function ArgChipBar({ specs, onSubmit, onCancel }: Props) {
	const [values, setValues] = useState<Record<string, string>>(() =>
		Object.fromEntries(specs.map((s) => [s.name, ""])),
	);
	const refs = useRef<(HTMLInputElement | null)[]>([]);
	const focusAt = (i: number) => refs.current[i]?.focus();

	return (
		<div className="lc-arg-bar">
			{specs.map((spec, i) => (
				<input
					key={spec.name}
					ref={(el) => {
						refs.current[i] = el;
					}}
					type={spec.kind.type === "number" ? "number" : "text"}
					inputMode={spec.kind.type === "number" ? "numeric" : undefined}
					placeholder={spec.placeholder}
					value={values[spec.name]}
					onChange={(e) =>
						setValues({ ...values, [spec.name]: e.target.value })
					}
					onKeyDown={(e) => {
						if (e.key === "Tab") {
							e.preventDefault();
							focusAt(
								e.shiftKey
									? Math.max(0, i - 1)
									: Math.min(specs.length - 1, i + 1),
							);
						} else if (e.key === "Enter") {
							const missing = specs.find((s) => s.required && !values[s.name]);
							if (missing) focusAt(specs.indexOf(missing));
							else onSubmit(values);
						} else if (e.key === "Backspace" && !values[spec.name]) {
							if (i === 0) onCancel();
							else focusAt(i - 1);
						}
					}}
					className="lc-arg-input"
				/>
			))}
		</div>
	);
}
