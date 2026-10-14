import { useState } from "react";

/** State for incrementally revealing a long list. */
export interface ShowMoreState {
	/** Number of items to render */
	limit: number;
	/** Number of items not yet rendered */
	remaining: number;
	/** Reveals the next page of items */
	showMore: () => void;
}

/**
 * Caps how many items of a long list are rendered, revealing more one page at a time.
 * Remount the owning component (e.g. via `key`) to reset it for a different list.
 * @param total - Total number of items in the list
 * @param pageSize - Number of items revealed initially and per `showMore` call
 * @returns Current limit, remaining count and the showMore action
 */
export function useShowMore(total: number, pageSize: number): ShowMoreState {
	const [limit, setLimit] = useState(pageSize);

	/** Reveals the next page of items. */
	function showMore(): void {
		setLimit((current) => current + pageSize);
	}

	return { limit, remaining: Math.max(0, total - limit), showMore };
}
