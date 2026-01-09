import { useEffect, useImperativeHandle, useRef, useState } from "react";

import { useTheme } from "../../contexts/useTheme";
import { ALL_FILTER_FLAGS, type AdvancedFilters, DEFAULT_ADVANCED_FILTERS, FILTER_OPTIONS, type ToolbarHandle } from "./constants";
import styles from "./Toolbar.module.scss";

interface ToolbarProps {
	ref?: React.Ref<ToolbarHandle>;
	searchQuery: string;
	activeFilters: string[];
	advancedFilters: AdvancedFilters;
	nodeCount: number;
	visibleCount: number;
	cycleCount: number;
	maxDepth: number;
	maxFanIn: number;
	maxFanOut: number;
	isExporting?: boolean;
	highlightCycles?: boolean;
	groupByFolder?: boolean;
	onSearchChange: (query: string) => void;
	onFiltersChange: (filters: string[]) => void;
	onAdvancedFiltersChange: (filters: AdvancedFilters) => void;
	onExportPng?: () => void;
	onExportSvg?: () => void;
	onExportJson?: () => void;
	onFitView?: () => void;
	onToggleCycles?: (highlight: boolean) => void;
	onToggleGroupByFolder?: (group: boolean) => void;
}

/**
 * Main toolbar with search, filters, and actions.
 * @param props - Component props
 * @returns Toolbar element
 */
export function Toolbar({
	ref,
	searchQuery,
	activeFilters,
	advancedFilters,
	nodeCount,
	visibleCount,
	cycleCount,
	maxDepth,
	maxFanIn,
	maxFanOut,
	isExporting,
	highlightCycles,
	groupByFolder,
	onSearchChange,
	onFiltersChange,
	onAdvancedFiltersChange,
	onExportPng,
	onExportSvg,
	onExportJson,
	onFitView,
	onToggleCycles,
	onToggleGroupByFolder,
}: ToolbarProps) {
	const [isFilterOpen, setIsFilterOpen] = useState(false);
	const [isExportOpen, setIsExportOpen] = useState(false);
	const [showAdvanced, setShowAdvanced] = useState(false);
	const filterRef = useRef<HTMLDivElement>(null);
	const exportRef = useRef<HTMLDivElement>(null);
	const searchInputRef = useRef<HTMLInputElement>(null);

	const hasAdvancedFilters = Object.values(advancedFilters).some((value) => value !== null);

	const allSelected = ALL_FILTER_FLAGS.every((flag) => activeFilters.includes(flag));
	const filterToggleClasses = [styles["filter-toggle"], isFilterOpen && styles["active"], (activeFilters.length > 0 || hasAdvancedFilters) && styles["has-filters"]]
		.filter(Boolean)
		.join(" ");

	/**
	 * Handles advanced filter input changes
	 * @param key The filter key
	 * @param value The new value
	 */
	function handleAdvancedFilterChange(key: keyof AdvancedFilters, value: string) {
		const numValue = value === "" ? null : parseInt(value, 10);

		onAdvancedFiltersChange({
			...advancedFilters,
			[key]: isNaN(numValue as number) ? null : numValue,
		});
	}

	/**
	 * Clears all advanced filters
	 */
	function handleClearAdvanced() {
		onAdvancedFiltersChange(DEFAULT_ADVANCED_FILTERS);
	}

	/**
	 * Handles toggling of filter flags
	 * @param flag The flag to toggle
	 */
	function handleFilterToggle(flag: string) {
		if (activeFilters.includes(flag)) {
			onFiltersChange(activeFilters.filter((f) => f !== flag));
		} else {
			onFiltersChange([...activeFilters, flag]);
		}
	}

	/**
	 * Selects all filter flags
	 */
	function handleSelectAll() {
		onFiltersChange(ALL_FILTER_FLAGS);
	}

	/**
	 * Clears the search input
	 */
	function handleClearSearch() {
		onSearchChange("");
	}

	useImperativeHandle(ref, () => ({
		focusSearch: () => {
			searchInputRef.current?.focus();
			searchInputRef.current?.select();
		},
	}));

	useEffect(() => {
		if (!isFilterOpen && !isExportOpen) return;

		/**
		 * Handles clicks outside of filter/export dropdowns to close them
		 * @param event - Mouse event
		 */
		function handleClickOutside(event: MouseEvent) {
			if (filterRef.current && !filterRef.current.contains(event.target as Node)) {
				setIsFilterOpen(false);
			}

			if (exportRef.current && !exportRef.current.contains(event.target as Node)) {
				setIsExportOpen(false);
			}
		}

		document.addEventListener("mousedown", handleClickOutside);

		return () => document.removeEventListener("mousedown", handleClickOutside);
	}, [isFilterOpen, isExportOpen]);

	return (
		<div className={styles["toolbar"]}>
			<div className={styles["search"]}>
				<svg className={styles["search-icon"]} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
					<circle cx="11" cy="11" r="8" />
					<line x1="21" y1="21" x2="16.65" y2="16.65" />
				</svg>

				<input
					ref={searchInputRef}
					type="text"
					placeholder="Search files... (press /)"
					value={searchQuery}
					onChange={(e) => onSearchChange(e.target.value)}
					className={styles["search-input"]}
				/>

				{searchQuery && (
					<button className={styles["clear-button"]} onClick={handleClearSearch} title="Clear search">
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
							<line x1="18" y1="6" x2="6" y2="18" />
							<line x1="6" y1="6" x2="18" y2="18" />
						</svg>
					</button>
				)}
			</div>

			<div className={styles["filters"]} ref={filterRef}>
				<button className={filterToggleClasses} onClick={() => setIsFilterOpen(!isFilterOpen)}>
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
						<polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3" />
					</svg>
					Filter
					{(activeFilters.length > 0 || hasAdvancedFilters) && <span className={styles["filter-count"]}>{activeFilters.length + (hasAdvancedFilters ? 1 : 0)}</span>}
				</button>

				{isFilterOpen && (
					<div className={`${styles["filter-dropdown"]} ${styles["wide"]}`}>
						<div className={styles["filter-header"]}>
							<span>Filter by flag</span>

							{!allSelected && (
								<button className={styles["clear-filters"]} onClick={handleSelectAll}>
									Select all
								</button>
							)}
						</div>

						<p className={styles["filter-hint"]}>{allSelected ? "Showing all nodes. Deselect flags to hide." : "Showing nodes matching selected flags."}</p>

						<div className={styles["filter-options"]}>
							{FILTER_OPTIONS.map(({ flag, label, className }) => (
								<label key={flag} className={styles["filter-option"]}>
									<input type="checkbox" checked={activeFilters.includes(flag)} onChange={() => handleFilterToggle(flag)} />
									<span className={`${styles["filter-swatch"]} ${styles[className]}`} />
									{label}
								</label>
							))}
						</div>

						<div className={styles["filter-section-divider"]}>
							<button className={styles["advanced-toggle"]} onClick={() => setShowAdvanced(!showAdvanced)}>
								<span>Advanced Filters</span>
								<svg
									width="12"
									height="12"
									viewBox="0 0 24 24"
									fill="none"
									stroke="currentColor"
									strokeWidth="2"
									style={{ transform: showAdvanced ? "rotate(180deg)" : "rotate(0deg)", transition: "transform 0.15s ease" }}
								>
									<polyline points="6 9 12 15 18 9" />
								</svg>
								{hasAdvancedFilters && <span className={styles["advanced-active-dot"]} />}
							</button>
						</div>

						{showAdvanced && (
							<div className={styles["advanced-filters"]}>
								{hasAdvancedFilters && (
									<button className={styles["clear-advanced"]} onClick={handleClearAdvanced}>
										Clear all
									</button>
								)}

								<div className={styles["filter-range-group"]}>
									<label className={styles["range-label"]}>Depth (0-{maxDepth})</label>

									<div className={styles["range-inputs"]}>
										<input
											type="number"
											placeholder="Min"
											min={0}
											max={maxDepth}
											value={advancedFilters.minDepth ?? ""}
											onChange={(e) => handleAdvancedFilterChange("minDepth", e.target.value)}
											className={styles["range-input"]}
										/>

										<span className={styles["range-separator"]}>to</span>

										<input
											type="number"
											placeholder="Max"
											min={0}
											max={maxDepth}
											value={advancedFilters.maxDepth ?? ""}
											onChange={(e) => handleAdvancedFilterChange("maxDepth", e.target.value)}
											className={styles["range-input"]}
										/>
									</div>
								</div>

								<div className={styles["filter-range-group"]}>
									<label className={styles["range-label"]}>Fan-In (0-{maxFanIn})</label>

									<div className={styles["range-inputs"]}>
										<input
											type="number"
											placeholder="Min"
											min={0}
											max={maxFanIn}
											value={advancedFilters.minFanIn ?? ""}
											onChange={(e) => handleAdvancedFilterChange("minFanIn", e.target.value)}
											className={styles["range-input"]}
										/>

										<span className={styles["range-separator"]}>to</span>

										<input
											type="number"
											placeholder="Max"
											min={0}
											max={maxFanIn}
											value={advancedFilters.maxFanIn ?? ""}
											onChange={(e) => handleAdvancedFilterChange("maxFanIn", e.target.value)}
											className={styles["range-input"]}
										/>
									</div>
								</div>

								<div className={styles["filter-range-group"]}>
									<label className={styles["range-label"]}>Fan-Out (0-{maxFanOut})</label>

									<div className={styles["range-inputs"]}>
										<input
											type="number"
											placeholder="Min"
											min={0}
											max={maxFanOut}
											value={advancedFilters.minFanOut ?? ""}
											onChange={(e) => handleAdvancedFilterChange("minFanOut", e.target.value)}
											className={styles["range-input"]}
										/>

										<span className={styles["range-separator"]}>to</span>

										<input
											type="number"
											placeholder="Max"
											min={0}
											max={maxFanOut}
											value={advancedFilters.maxFanOut ?? ""}
											onChange={(e) => handleAdvancedFilterChange("maxFanOut", e.target.value)}
											className={styles["range-input"]}
										/>
									</div>
								</div>
							</div>
						)}
					</div>
				)}
			</div>

			<div className={styles["stats"]}>
				{visibleCount < nodeCount ? (
					<span>
						Showing {visibleCount} of {nodeCount} files
					</span>
				) : (
					<span>{nodeCount} files</span>
				)}
			</div>

			{onFitView && (
				<button className={styles["fit-view-button"]} onClick={onFitView} title="Fit to screen" aria-label="Fit to screen">
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
						<path d="M8 3H5a2 2 0 0 0-2 2v3" />
						<path d="M21 8V5a2 2 0 0 0-2-2h-3" />
						<path d="M3 16v3a2 2 0 0 0 2 2h3" />
						<path d="M16 21h3a2 2 0 0 0 2-2v-3" />
					</svg>

					<span>Fit View</span>
				</button>
			)}

			{onToggleCycles && cycleCount > 0 && (
				<button
					className={`${styles["cycle-button"]} ${highlightCycles ? styles["active"] : ""}`}
					onClick={() => onToggleCycles(!highlightCycles)}
					title={highlightCycles ? "Hide cycles" : "Highlight cycles"}
					aria-label={highlightCycles ? "Hide cycles" : "Highlight cycles"}
				>
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
						<circle cx="12" cy="12" r="10" />
						<path d="M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0z" />
						<path d="M12 8v-4" />
						<path d="M12 20v-4" />
					</svg>
					<span>Cycles ({cycleCount})</span>
				</button>
			)}

			{onToggleGroupByFolder && (
				<button
					className={`${styles["group-button"]} ${groupByFolder ? styles["active"] : ""}`}
					onClick={() => onToggleGroupByFolder(!groupByFolder)}
					title={groupByFolder ? "Ungroup files" : "Group by folder"}
					aria-label={groupByFolder ? "Ungroup files" : "Group by folder"}
				>
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
						<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
					</svg>

					<span>Group</span>
				</button>
			)}

			{(onExportPng || onExportSvg || onExportJson) && (
				<div className={styles["export"]} ref={exportRef}>
					<button
						className={`${styles["export-toggle"]} ${isExportOpen ? styles["active"] : ""} ${isExporting ? styles["exporting"] : ""}`}
						onClick={() => setIsExportOpen(!isExportOpen)}
						disabled={isExporting}
						title="Export options"
						aria-label="Export options"
					>
						{isExporting ? (
							<svg className={styles["spinner"]} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
								<circle cx="12" cy="12" r="10" strokeDasharray="32" strokeDashoffset="12" />
							</svg>
						) : (
							<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
								<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
								<polyline points="7 10 12 15 17 10" />
								<line x1="12" y1="15" x2="12" y2="3" />
							</svg>
						)}

						<span>{isExporting ? "Exporting..." : "Export"}</span>

						<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
							<polyline points="6 9 12 15 18 9" />
						</svg>
					</button>

					{isExportOpen && (
						<div className={styles["export-dropdown"]}>
							<div className={styles["export-header"]}>
								<span>Export Format</span>
							</div>

							{onExportPng && (
								<button
									className={styles["export-option"]}
									onClick={() => {
										onExportPng();
										setIsExportOpen(false);
									}}
								>
									<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
										<rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
										<circle cx="8.5" cy="8.5" r="1.5" />
										<polyline points="21 15 16 10 5 21" />
									</svg>

									<span>PNG Image</span>

									<span className={styles["export-hint"]}>High resolution</span>
								</button>
							)}

							{onExportSvg && (
								<button
									className={styles["export-option"]}
									onClick={() => {
										onExportSvg();
										setIsExportOpen(false);
									}}
								>
									<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
										<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
										<polyline points="14 2 14 8 20 8" />
									</svg>

									<span>SVG Vector</span>

									<span className={styles["export-hint"]}>Scalable</span>
								</button>
							)}

							{onExportJson && (
								<button
									className={styles["export-option"]}
									onClick={() => {
										onExportJson();
										setIsExportOpen(false);
									}}
								>
									<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
										<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
										<polyline points="14 2 14 8 20 8" />
										<line x1="16" y1="13" x2="8" y2="13" />
										<line x1="16" y1="17" x2="8" y2="17" />
									</svg>

									<span>JSON Subgraph</span>

									<span className={styles["export-hint"]}>Visible nodes only</span>
								</button>
							)}
						</div>
					)}
				</div>
			)}

			<ThemeToggle />
		</div>
	);
}

/**
 * Theme toggle button for light/dark mode.
 * @returns Theme toggle button
 */
function ThemeToggle() {
	const { theme, toggleTheme } = useTheme();

	return (
		<button
			className={styles["theme-toggle"]}
			onClick={toggleTheme}
			title={`Switch to ${theme === "light" ? "dark" : "light"} mode`}
			aria-label={`Switch to ${theme === "light" ? "dark" : "light"} mode`}
		>
			{theme === "light" ? (
				<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
					<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
				</svg>
			) : (
				<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
					<circle cx="12" cy="12" r="5" />
					<line x1="12" y1="1" x2="12" y2="3" />
					<line x1="12" y1="21" x2="12" y2="23" />
					<line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
					<line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
					<line x1="1" y1="12" x2="3" y2="12" />
					<line x1="21" y1="12" x2="23" y2="12" />
					<line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
					<line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
				</svg>
			)}
		</button>
	);
}
