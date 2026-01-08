import { useEffect, useCallback } from "react";

interface KeyboardShortcutsOptions {
	onEscape: () => void;
	onFocusSearch: () => void;
	onFitView: () => void;
	onClearSelection: () => void;
}

/**
 * Registers global keyboard shortcuts for graph navigation.
 * - `/` or `Cmd+K`: Focus search
 * - `f`: Fit view
 * - `Esc`: Clear selection and search
 */
export function useKeyboardShortcuts({
	onEscape,
	onFocusSearch,
	onFitView,
	onClearSelection,
}: KeyboardShortcutsOptions): void {
	const handleKeyDown = useCallback(
		(event: KeyboardEvent) => {
			const target = event.target as HTMLElement;
			const isTyping = target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;

			if (event.key === "Escape") {
				if (isTyping) {
					target.blur();
				}
				onEscape();
				onClearSelection();
				return;
			}

			if (isTyping) return;

			if (event.key === "/" || ((event.metaKey || event.ctrlKey) && event.key === "k")) {
				event.preventDefault();
				onFocusSearch();
				return;
			}

			if (event.key === "f") {
				event.preventDefault();
				onFitView();
				return;
			}
		},
		[onFocusSearch, onFitView, onClearSelection, onEscape],
	);

	useEffect(() => {
		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	}, [handleKeyDown]);
}
