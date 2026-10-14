import { useState } from "react";

import { ShowMoreButton } from "./ShowMoreButton";
import { useShowMore } from "./useShowMore";
import type { SassDepOutput, UnresolvedImport } from "../../types/sass-dep";
import styles from "./Sidebar.module.scss";

/** Number of unresolved imports / cycles rendered per page. */
const LIST_PAGE_SIZE = 20;

/** Props for the Overview component. */
interface OverviewProps {
	/** Normalized sass-dep output */
	data: SassDepOutput;
	/** Called when a file is clicked */
	onFocusNode?: (nodeId: string) => void;
}

/** Props for the CollapsibleSection component. */
interface CollapsibleSectionProps {
	/** Section title */
	title: string;
	/** Item count shown next to the title */
	count: number;
	/** Whether the section starts expanded */
	defaultOpen?: boolean;
	/** Section body */
	children: React.ReactNode;
}

/**
 * Detail group with a header button that expands/collapses its body.
 * @param props - Component props
 * @returns Collapsible section
 */
function CollapsibleSection({ title, count, defaultOpen = true, children }: CollapsibleSectionProps) {
	const [isOpen, setIsOpen] = useState(defaultOpen);

	/** Toggles the section open/closed. */
	function handleToggle() {
		setIsOpen((prev) => !prev);
	}

	return (
		<div className={styles["detail-group"]}>
			<button type="button" className={styles["collapsible-header"]} onClick={handleToggle} aria-expanded={isOpen}>
				<svg
					width="10"
					height="10"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					strokeWidth="2"
					style={{ transform: isOpen ? "rotate(90deg)" : "rotate(0deg)" }}
				>
					<polyline points="9 18 15 12 9 6" />
				</svg>
				{title}
				<span className={styles["label-count"]}>{count}</span>
			</button>
			{isOpen && children}
		</div>
	);
}

/** Props for the UnresolvedList component. */
interface UnresolvedListProps {
	/** Unresolved imports */
	entries: UnresolvedImport[];
	/** Called when a file is clicked */
	onFocusNode?: (nodeId: string) => void;
}

/**
 * Capped list of unresolved imports rendered as `from → target @ line:col`.
 * @param props - Component props
 * @returns List element
 */
function UnresolvedList({ entries, onFocusNode }: UnresolvedListProps) {
	const { limit, remaining, showMore } = useShowMore(entries.length, LIST_PAGE_SIZE);

	return (
		<ul className={styles["issue-list"]}>
			{entries.slice(0, limit).map((entry, i) => (
				<li key={`${entry.from}:${entry.location.line}:${entry.location.column}:${i}`} className={styles["issue-item"]}>
					<button type="button" className={styles["inline-file"]} onClick={() => onFocusNode?.(entry.from)} title={`Focus on ${entry.from}`}>
						{entry.from}
					</button>
					<span className={styles["issue-arrow"]}>→</span>
					<code className={styles["issue-target"]}>{entry.target}</code>
					<span className={styles["issue-meta"]}>
						@{entry.directive_type} · {entry.location.line}:{entry.location.column}
						{entry.reason ? ` · ${entry.reason}` : ""}
					</span>
				</li>
			))}
			{remaining > 0 && (
				<li>
					<ShowMoreButton remaining={remaining} pageSize={LIST_PAGE_SIZE} onClick={showMore} />
				</li>
			)}
		</ul>
	);
}

/** Props for the CycleList component. */
interface CycleListProps {
	/** Cycles; each is an ordered closed path */
	cycles: string[][];
	/** Called when a file is clicked */
	onFocusNode?: (nodeId: string) => void;
}

/**
 * Capped list of cycles, each rendered as `a → b → c → a` with clickable files.
 * @param props - Component props
 * @returns List element
 */
function CycleList({ cycles, onFocusNode }: CycleListProps) {
	const { limit, remaining, showMore } = useShowMore(cycles.length, LIST_PAGE_SIZE);

	return (
		<ul className={styles["issue-list"]}>
			{cycles.slice(0, limit).map((cycle, i) => (
				<li key={i} className={styles["cycle-item"]}>
					{[...cycle, cycle[0]!].map((fileId, step) => (
						<span key={step} className={styles["cycle-step"]}>
							{step > 0 && <span className={styles["issue-arrow"]}>→</span>}
							<button type="button" className={styles["inline-file"]} onClick={() => onFocusNode?.(fileId)} title={`Focus on ${fileId}`}>
								{fileId}
							</button>
						</span>
					))}
				</li>
			))}
			{remaining > 0 && (
				<li>
					<ShowMoreButton remaining={remaining} pageSize={LIST_PAGE_SIZE} onClick={showMore} />
				</li>
			)}
		</ul>
	);
}

/** Props for the StatItem component. */
interface StatItemProps {
	/** Statistic label */
	label: string;
	/** Statistic value */
	value: number;
	/** Whether a non-zero value should be shown as a warning */
	warnWhenNonZero?: boolean;
	/** Tooltip */
	title?: string;
}

/**
 * Single statistic tile.
 * @param props - Component props
 * @returns Statistic element
 */
function StatItem({ label, value, warnWhenNonZero = false, title }: StatItemProps) {
	const className = [styles["stat-item"], warnWhenNonZero && value > 0 && styles["stat-warning"]].filter(Boolean).join(" ");

	return (
		<div className={className} title={title}>
			<dt>{label}</dt>
			<dd>{value}</dd>
		</div>
	);
}

/**
 * Project overview shown when nothing is selected: statistics, legacy @import hint,
 * unresolved imports and cycles.
 * @param props - Component props
 * @returns Overview section
 */
export function Overview({ data, onFocusNode }: OverviewProps) {
	const { statistics, cycles, unresolved } = data.analysis;

	return (
		<div className={styles["section"]}>
			<h3 className={styles["section-title"]}>Statistics</h3>
			<dl className={styles["stats-list"]}>
				<StatItem label="Total Files" value={statistics.total_files} />
				<StatItem label="Dependencies" value={statistics.total_dependencies} />
				<StatItem label="Entry Points" value={statistics.entry_points} />
				<StatItem label="Orphan Files" value={statistics.orphan_files} />
				<StatItem label="Leaf Files" value={statistics.leaf_files} />
				<StatItem label="Max Depth" value={statistics.max_depth} />
				<StatItem label="Max Fan-In" value={statistics.max_fan_in} />
				<StatItem label="Max Fan-Out" value={statistics.max_fan_out} />
				<StatItem label="Cycles" value={statistics.cycle_count} warnWhenNonZero title="Circular dependency chains" />
				<StatItem label="Unresolved" value={statistics.unresolved_imports} warnWhenNonZero title="Imports that could not be resolved to a file" />
				<StatItem label="Legacy @import" value={statistics.import_directives} warnWhenNonZero title="Number of @import directives" />
			</dl>

			{statistics.import_directives > 0 && (
				<p className={styles["callout"]}>
					<strong>@import is deprecated in Dart Sass</strong> and will be removed in Dart Sass 3.0. Migrate to <code>@use</code> and <code>@forward</code>.
				</p>
			)}

			{unresolved.length > 0 && (
				<CollapsibleSection title="Unresolved imports" count={unresolved.length}>
					<UnresolvedList entries={unresolved} onFocusNode={onFocusNode} />
				</CollapsibleSection>
			)}

			{cycles.length > 0 && (
				<CollapsibleSection title="Cycles" count={cycles.length}>
					<CycleList cycles={cycles} onFocusNode={onFocusNode} />
				</CollapsibleSection>
			)}

			<p className={styles["hint"]}>Click on a node or edge to see details. Shift-click two nodes to find the dependency path between them.</p>
		</div>
	);
}
