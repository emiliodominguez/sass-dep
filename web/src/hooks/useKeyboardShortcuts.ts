import { useEffect, useCallback } from "react";

interface KeyboardShortcutsOptions {
	onFocusSearch: () => void;
	onFitView: () => void;
	onClearSelection: () => void;
	onEscape: () => void;
}

export function useKeyboardShortcuts({ onFocusSearch, onFitView, onClearSelection, onEscape }: KeyboardShortcutsOptions) {
	const handleKeyDown = useCallback(
		(event: KeyboardEvent) => {
			// Ignore if user is typing in an input
			const target = event.target as HTMLElement;
			const isTyping = target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;

			// Escape always works, even in inputs
			if (event.key === "Escape") {
				if (isTyping) {
					// Blur the input
					target.blur();
				}
				onEscape();
				onClearSelection();
				return;
			}

			// Other shortcuts only work when not typing
			if (isTyping) return;

			// "/" or "Cmd+K" / "Ctrl+K" to focus search
			if (event.key === "/" || ((event.metaKey || event.ctrlKey) && event.key === "k")) {
				event.preventDefault();
				onFocusSearch();
				return;
			}

			// "f" to fit view
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
