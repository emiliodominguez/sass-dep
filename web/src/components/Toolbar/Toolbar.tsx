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

interface ToolbarProps {
	searchQuery: string;
	activeFilters: string[];
	nodeCount: number;
	visibleCount: number;
	isExporting?: boolean;
	onSearchChange: (query: string) => void;
	onFiltersChange: (filters: string[]) => void;
	onExportPng?: () => void;
	onFitView?: () => void;
}

export const Toolbar = forwardRef<ToolbarHandle, ToolbarProps>(function Toolbar(
	{ searchQuery, activeFilters, nodeCount, visibleCount, isExporting, onSearchChange, onFiltersChange, onExportPng, onFitView },
	ref,
) {
	const [isFilterOpen, setIsFilterOpen] = useState(false);
	const filterRef = useRef<HTMLDivElement>(null);
	const searchInputRef = useRef<HTMLInputElement>(null);

	useImperativeHandle(ref, () => ({
		focusSearch: () => {
			searchInputRef.current?.focus();
			searchInputRef.current?.select();
		},
	}));

	// Close dropdown when clicking outside
	useEffect(() => {
		if (!isFilterOpen) return;

		const handleClickOutside = (event: MouseEvent) => {
			if (filterRef.current && !filterRef.current.contains(event.target as Node)) {
				setIsFilterOpen(false);
			}
		};

		document.addEventListener("mousedown", handleClickOutside);
		return () => document.removeEventListener("mousedown", handleClickOutside);
	}, [isFilterOpen]);

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
					className={`filter-toggle ${isFilterOpen ? "active" : ""} ${activeFilters.length > 0 ? "has-filters" : ""}`}
					onClick={() => setIsFilterOpen(!isFilterOpen)}
				>
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
						<polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3" />
					</svg>
					Filter
					{activeFilters.length > 0 && <span className="filter-count">{activeFilters.length}</span>}
				</button>

				{isFilterOpen && (
					<div className="filter-dropdown">
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

			{onExportPng && (
				<button
					className={`export-button ${isExporting ? "exporting" : ""}`}
					onClick={onExportPng}
					disabled={isExporting}
					title="Export as PNG"
					aria-label="Export as PNG"
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
					<span>{isExporting ? "Exporting..." : "Export PNG"}</span>
				</button>
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
