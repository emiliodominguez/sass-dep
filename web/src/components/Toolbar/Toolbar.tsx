import { useState, useCallback, useEffect, useRef, forwardRef, useImperativeHandle } from "react";
import type { NodeFlag } from "../../types/sass-dep";
import { useTheme } from "../../contexts/ThemeContext";
import "./Toolbar.css";

export interface ToolbarHandle {
	focusSearch: () => void;
}

export const FILTER_OPTIONS: { flag: NodeFlag | "no_flags"; label: string }[] = [
	{ flag: "entry_point", label: "Entry Points" },
	{ flag: "in_cycle", label: "In Cycle" },
	{ flag: "orphan", label: "Orphans" },
	{ flag: "leaf", label: "Leaves" },
	{ flag: "high_fan_in", label: "High Fan-In" },
	{ flag: "high_fan_out", label: "High Fan-Out" },
	{ flag: "no_flags", label: "No Flags" },
];

export const ALL_FILTER_FLAGS = FILTER_OPTIONS.map((o) => o.flag);

export interface AdvancedFilters {
	minDepth: number | null;
	maxDepth: number | null;
	minFanIn: number | null;
	maxFanIn: number | null;
	minFanOut: number | null;
	maxFanOut: number | null;
}

export const DEFAULT_ADVANCED_FILTERS: AdvancedFilters = {
	minDepth: null,
	maxDepth: null,
	minFanIn: null,
	maxFanIn: null,
	minFanOut: null,
	maxFanOut: null,
};

interface ToolbarProps {
	// Data props
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
	// Callbacks
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

export const Toolbar = forwardRef<ToolbarHandle, ToolbarProps>(function Toolbar(
	{ searchQuery, activeFilters, advancedFilters, nodeCount, visibleCount, cycleCount, maxDepth, maxFanIn, maxFanOut, isExporting, highlightCycles, groupByFolder, onSearchChange, onFiltersChange, onAdvancedFiltersChange, onExportPng, onExportSvg, onExportJson, onFitView, onToggleCycles, onToggleGroupByFolder },
	ref,
) {
	const [isFilterOpen, setIsFilterOpen] = useState(false);
	const [isExportOpen, setIsExportOpen] = useState(false);
	const [showAdvanced, setShowAdvanced] = useState(false);
	const filterRef = useRef<HTMLDivElement>(null);
	const exportRef = useRef<HTMLDivElement>(null);
	const searchInputRef = useRef<HTMLInputElement>(null);

	// Check if any advanced filters are active
	const hasAdvancedFilters = Object.values(advancedFilters).some((v) => v !== null);

	const handleAdvancedFilterChange = useCallback(
		(key: keyof AdvancedFilters, value: string) => {
			const numValue = value === "" ? null : parseInt(value, 10);
			onAdvancedFiltersChange({ ...advancedFilters, [key]: isNaN(numValue as number) ? null : numValue });
		},
		[advancedFilters, onAdvancedFiltersChange],
	);

	const handleClearAdvanced = useCallback(() => {
		onAdvancedFiltersChange(DEFAULT_ADVANCED_FILTERS);
	}, [onAdvancedFiltersChange]);

	useImperativeHandle(ref, () => ({
		focusSearch: () => {
			searchInputRef.current?.focus();
			searchInputRef.current?.select();
		},
	}));

	// Close dropdowns when clicking outside
	useEffect(() => {
		if (!isFilterOpen && !isExportOpen) return;

		const handleClickOutside = (event: MouseEvent) => {
			if (filterRef.current && !filterRef.current.contains(event.target as Node)) {
				setIsFilterOpen(false);
			}
			if (exportRef.current && !exportRef.current.contains(event.target as Node)) {
				setIsExportOpen(false);
			}
		};

		document.addEventListener("mousedown", handleClickOutside);
		return () => document.removeEventListener("mousedown", handleClickOutside);
	}, [isFilterOpen, isExportOpen]);

	const handleFilterToggle = useCallback(
		(flag: string) => {
			if (activeFilters.includes(flag)) {
				onFiltersChange(activeFilters.filter((f) => f !== flag));
			} else {
				onFiltersChange([...activeFilters, flag]);
			}
		},
		[activeFilters, onFiltersChange],
	);

	const handleSelectAll = useCallback(() => {
		onFiltersChange(ALL_FILTER_FLAGS);
	}, [onFiltersChange]);

	const allSelected = ALL_FILTER_FLAGS.every((flag) => activeFilters.includes(flag));

	const handleClearSearch = useCallback(() => {
		onSearchChange("");
	}, [onSearchChange]);

	return (
		<div className="toolbar">
			<div className="toolbar-search">
				<svg className="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
					<circle cx="11" cy="11" r="8" />
					<line x1="21" y1="21" x2="16.65" y2="16.65" />
				</svg>
				<input ref={searchInputRef} type="text" placeholder="Search files... (press /)" value={searchQuery} onChange={(e) => onSearchChange(e.target.value)} className="search-input" />
				{searchQuery && (
					<button className="clear-button" onClick={handleClearSearch} title="Clear search">
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
							<line x1="18" y1="6" x2="6" y2="18" />
							<line x1="6" y1="6" x2="18" y2="18" />
						</svg>
					</button>
				)}
			</div>

			<div className="toolbar-filters" ref={filterRef}>
				<button
					className={`filter-toggle ${isFilterOpen ? "active" : ""} ${activeFilters.length > 0 || hasAdvancedFilters ? "has-filters" : ""}`}
					onClick={() => setIsFilterOpen(!isFilterOpen)}
				>
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
						<polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3" />
					</svg>
					Filter
					{(activeFilters.length > 0 || hasAdvancedFilters) && (
						<span className="filter-count">{activeFilters.length + (hasAdvancedFilters ? 1 : 0)}</span>
					)}
				</button>

				{isFilterOpen && (
					<div className="filter-dropdown filter-dropdown-wide">
						<div className="filter-header">
							<span>Filter by flag</span>
							{!allSelected && (
								<button className="clear-filters" onClick={handleSelectAll}>
									Select all
								</button>
							)}
						</div>
						<p className="filter-hint">{allSelected ? "Showing all nodes. Deselect flags to hide." : "Showing nodes matching selected flags."}</p>
						<div className="filter-options">
							{FILTER_OPTIONS.map(({ flag, label }) => (
								<label key={flag} className="filter-option">
									<input type="checkbox" checked={activeFilters.includes(flag)} onChange={() => handleFilterToggle(flag)} />
									<span className={`filter-swatch filter-swatch-${flag}`} />
									{label}
								</label>
							))}
						</div>

						{/* Advanced Filters Section */}
						<div className="filter-section-divider">
							<button className="advanced-toggle" onClick={() => setShowAdvanced(!showAdvanced)}>
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
								{hasAdvancedFilters && <span className="advanced-active-dot" />}
							</button>
						</div>

						{showAdvanced && (
							<div className="advanced-filters">
								{hasAdvancedFilters && (
									<button className="clear-advanced" onClick={handleClearAdvanced}>
										Clear all
									</button>
								)}

								<div className="filter-range-group">
									<label className="range-label">Depth (0-{maxDepth})</label>
									<div className="range-inputs">
										<input
											type="number"
											placeholder="Min"
											min={0}
											max={maxDepth}
											value={advancedFilters.minDepth ?? ""}
											onChange={(e) => handleAdvancedFilterChange("minDepth", e.target.value)}
											className="range-input"
										/>
										<span className="range-separator">to</span>
										<input
											type="number"
											placeholder="Max"
											min={0}
											max={maxDepth}
											value={advancedFilters.maxDepth ?? ""}
											onChange={(e) => handleAdvancedFilterChange("maxDepth", e.target.value)}
											className="range-input"
										/>
									</div>
								</div>

								<div className="filter-range-group">
									<label className="range-label">Fan-In (0-{maxFanIn})</label>
									<div className="range-inputs">
										<input
											type="number"
											placeholder="Min"
											min={0}
											max={maxFanIn}
											value={advancedFilters.minFanIn ?? ""}
											onChange={(e) => handleAdvancedFilterChange("minFanIn", e.target.value)}
											className="range-input"
										/>
										<span className="range-separator">to</span>
										<input
											type="number"
											placeholder="Max"
											min={0}
											max={maxFanIn}
											value={advancedFilters.maxFanIn ?? ""}
											onChange={(e) => handleAdvancedFilterChange("maxFanIn", e.target.value)}
											className="range-input"
										/>
									</div>
								</div>

								<div className="filter-range-group">
									<label className="range-label">Fan-Out (0-{maxFanOut})</label>
									<div className="range-inputs">
										<input
											type="number"
											placeholder="Min"
											min={0}
											max={maxFanOut}
											value={advancedFilters.minFanOut ?? ""}
											onChange={(e) => handleAdvancedFilterChange("minFanOut", e.target.value)}
											className="range-input"
										/>
										<span className="range-separator">to</span>
										<input
											type="number"
											placeholder="Max"
											min={0}
											max={maxFanOut}
											value={advancedFilters.maxFanOut ?? ""}
											onChange={(e) => handleAdvancedFilterChange("maxFanOut", e.target.value)}
											className="range-input"
										/>
									</div>
								</div>
							</div>
						)}
					</div>
				)}
			</div>

			<div className="toolbar-stats">
				{visibleCount < nodeCount ? (
					<span>
						Showing {visibleCount} of {nodeCount} files
					</span>
				) : (
					<span>{nodeCount} files</span>
				)}
			</div>

			{onFitView && (
				<button className="fit-view-button" onClick={onFitView} title="Fit to screen" aria-label="Fit to screen">
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
					className={`cycle-button ${highlightCycles ? "active" : ""}`}
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
					className={`group-button ${groupByFolder ? "active" : ""}`}
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
				<div className="toolbar-export" ref={exportRef}>
					<button
						className={`export-toggle ${isExportOpen ? "active" : ""} ${isExporting ? "exporting" : ""}`}
						onClick={() => setIsExportOpen(!isExportOpen)}
						disabled={isExporting}
						title="Export options"
						aria-label="Export options"
					>
						{isExporting ? (
							<svg className="spinner" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
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
						<div className="export-dropdown">
							<div className="export-header">
								<span>Export Format</span>
							</div>
							{onExportPng && (
								<button
									className="export-option"
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
									<span className="export-hint">High resolution</span>
								</button>
							)}
							{onExportSvg && (
								<button
									className="export-option"
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
									<span className="export-hint">Scalable</span>
								</button>
							)}
							{onExportJson && (
								<button
									className="export-option"
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
									<span className="export-hint">Visible nodes only</span>
								</button>
							)}
						</div>
					)}
				</div>
			)}

			<ThemeToggle />
		</div>
	);
});

function ThemeToggle() {
	const { theme, toggleTheme } = useTheme();

	return (
		<button
			className="theme-toggle"
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
