import { useCallback, useState } from "react";

export function useSetToggle(initial?: Iterable<string>) {
	const [set, setSet] = useState<Set<string>>(() => new Set(initial));
	const toggle = useCallback((id: string) => {
		setSet((prev) => {
			const next = new Set(prev);
			if (next.has(id)) next.delete(id);
			else next.add(id);
			return next;
		});
	}, []);
	return [set, toggle] as const;
}
