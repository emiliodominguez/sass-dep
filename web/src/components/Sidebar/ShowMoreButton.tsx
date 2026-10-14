import styles from "./Sidebar.module.scss";

/** Props for the ShowMoreButton component. */
interface ShowMoreButtonProps {
	/** Number of items not yet rendered */
	remaining: number;
	/** Number of items revealed per click */
	pageSize: number;
	/** Reveals the next page */
	onClick: () => void;
}

/**
 * "Show N more" affordance for capped lists. Renders nothing when no items are hidden.
 * @param props - Component props
 * @returns Button element or null
 */
export function ShowMoreButton({ remaining, pageSize, onClick }: ShowMoreButtonProps) {
	if (remaining <= 0) return null;

	const next = Math.min(remaining, pageSize);

	return (
		<button type="button" className={styles["show-more"]} onClick={onClick}>
			Show {next} more{remaining > next ? ` (${remaining} hidden)` : ""}
		</button>
	);
}
