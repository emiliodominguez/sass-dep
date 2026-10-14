import { useEffect } from "react";

/** Callbacks invoked by the global keyboard shortcuts. */
interface KeyboardShortcutsOptions {
	/** Called on `Esc` (clears the search) */
	onEscape: () => void;
	/** Called on `/` or `Cmd/Ctrl+K` */
	onFocusSearch: () => void;
	/** Called on `f` */
	onFitView: () => void;
	/** Called on `Esc` (clears selection, path and focus) */
	onClearSelection: () => void;
}

/**
 * Checks whether a keyboard event originates from a text-editing element.
 * @param target - Event target
 * @returns True when the user is typing in an input, textarea or contenteditable element
 */
function isTypingTarget(target: EventTarget | null): target is HTMLElement {
	return target instanceof HTMLElement && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);
}

/**
 * Registers global keyboard shortcuts for graph navigation.
 * - `/` or `Cmd/Ctrl+K`: Focus search
 * - `f`: Fit view
 * - `Esc`: Clear selection and search
 * @param options - Shortcut callbacks
 */
export function useKeyboardShortcuts({ onEscape, onFocusSearch, onFitView, onClearSelection }: KeyboardShortcutsOptions): void {
	useEffect(() => {
		/**
		 * Dispatches a keydown event to the matching shortcut.
		 * @param event - Keyboard event
		 */
		function handleKeyDown(event: KeyboardEvent) {
			const isTyping = isTypingTarget(event.target);

			if (event.key === "Escape") {
				if (isTyping) {
					event.target.blur();
				}
				onEscape();
				onClearSelection();
				return;
			}

			if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
				event.preventDefault();
				onFocusSearch();
				return;
			}

			if (isTyping || event.metaKey || event.ctrlKey || event.altKey) return;

			if (event.key === "/") {
				event.preventDefault();
				onFocusSearch();
				return;
			}

			if (event.key === "f") {
				event.preventDefault();
				onFitView();
			}
		}

		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	}, [onFocusSearch, onFitView, onClearSelection, onEscape]);
}
