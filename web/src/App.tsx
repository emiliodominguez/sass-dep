import { useMemo, useRef, useState } from "react";

import { useGraphData } from "./hooks/useGraphData";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { Graph, type GraphHandle } from "./components/Graph/Graph";
import { Sidebar } from "./components/Sidebar/Sidebar";
import { DataLoader } from "./components/DataLoader/DataLoader";
import { Legend } from "./components/Legend/Legend";
import { Toolbar } from "./components/Toolbar/Toolbar";
import { ALL_FILTER_FLAGS, type AdvancedFilters, DEFAULT_ADVANCED_FILTERS, type ToolbarHandle } from "./components/Toolbar/constants";
import type { OutputEdge, OutputNode } from "./types/sass-dep";
import styles from "./App.module.scss";

/**
 * Main application component.
 * @returns App with graph visualization or data loader
 */
function App() {
	const { data, isLoading, setData } = useGraphData();
	const [selectedNode, setSelectedNode] = useState<{ id: string; node: OutputNode } | null>(null);
	const [selectedEdge, setSelectedEdge] = useState<OutputEdge | null>(null);
	const [searchQuery, setSearchQuery] = useState("");
	const [activeFilters, setActiveFilters] = useState<string[]>(ALL_FILTER_FLAGS);
	const [advancedFilters, setAdvancedFilters] = useState<AdvancedFilters>(DEFAULT_ADVANCED_FILTERS);
	const graphRef = useRef<GraphHandle>(null);
	const toolbarRef = useRef<ToolbarHandle>(null);

	const [pathSource, setPathSource] = useState<string | null>(null);
	const [pathTarget, setPathTarget] = useState<string | null>(null);
	const [highlightCycles, setHighlightCycles] = useState(false);
	const [groupByFolder, setGroupByFolder] = useState(false);
	const [isExporting, setIsExporting] = useState(false);

	const { nodeCount, visibleCount } = useMemo(() => {
		if (!data) return { nodeCount: 0, visibleCount: 0 };

		const nodes = Object.entries(data.nodes);
		const totalCount = nodes.length;

		const visibleNodeCount = nodes.filter(([id, node]) => {
			const matchesSearch = !searchQuery || id.toLowerCase().includes(searchQuery.toLowerCase()) || node.path.toLowerCase().includes(searchQuery.toLowerCase());

			const matchesFilters =
				activeFilters.length === 0 ||
				activeFilters.some((filter) => {
					if (filter === "no_flags") {
						return node.flags.length === 0;
					}
					return node.flags.includes(filter as OutputNode["flags"][number]);
				});

			const { minDepth, maxDepth, minFanIn, maxFanIn, minFanOut, maxFanOut } = advancedFilters;
			const { depth, fan_in, fan_out } = node.metrics;

			const matchesAdvanced =
				(minDepth === null || depth >= minDepth) &&
				(maxDepth === null || depth <= maxDepth) &&
				(minFanIn === null || fan_in >= minFanIn) &&
				(maxFanIn === null || fan_in <= maxFanIn) &&
				(minFanOut === null || fan_out >= minFanOut) &&
				(maxFanOut === null || fan_out <= maxFanOut);

			return matchesSearch && matchesFilters && matchesAdvanced;
		}).length;

		return { nodeCount: totalCount, visibleCount: visibleNodeCount };
	}, [data, searchQuery, activeFilters, advancedFilters]);

	/**
	 * Handles node selection, supporting shift-click for path highlighting.
	 * @param nodeId - The selected node's ID
	 * @param node - The selected node data
	 * @param isShiftClick - Whether shift key was held during click
	 */
	function handleNodeSelect(nodeId: string, node: OutputNode, isShiftClick?: boolean) {
		if (isShiftClick) {
			if (!pathSource) {
				setPathSource(nodeId);
				setPathTarget(null);
			} else if (pathSource === nodeId) {
				setPathSource(null);
				setPathTarget(null);
			} else {
				setPathTarget(nodeId);
			}
		} else {
			setPathSource(null);
			setPathTarget(null);
		}

		setSelectedNode({ id: nodeId, node });
		setSelectedEdge(null);
	}

	/**
	 * Handles edge selection.
	 * @param edge - The selected edge data
	 */
	function handleEdgeSelect(edge: OutputEdge) {
		setSelectedEdge(edge);
		setSelectedNode(null);
	}

	/**
	 * Clears the current selection (node, edge, and path).
	 */
	function handleClearSelection() {
		setSelectedNode(null);
		setSelectedEdge(null);
		setPathSource(null);
		setPathTarget(null);
	}

	/**
	 * Focuses the graph view on a specific node.
	 * @param nodeId - The node ID to focus on
	 */
	function handleFocusNode(nodeId: string) {
		graphRef.current?.focusNode(nodeId);
	}

	/**
	 * Fits the graph view to show all visible nodes.
	 */
	function handleFitView() {
		graphRef.current?.fitView();
	}

	/**
	 * Focuses the search input in the toolbar.
	 */
	function handleFocusSearch() {
		toolbarRef.current?.focusSearch();
	}

	/**
	 * Exports the current graph view as a PNG image.
	 */
	async function handleExportPng() {
		if (!graphRef.current) return;

		setIsExporting(true);

		try {
			await graphRef.current.exportPng();
		} finally {
			setIsExporting(false);
		}
	}

	/**
	 * Exports the current graph view as an SVG file.
	 */
	async function handleExportSvg() {
		if (!graphRef.current) return;

		setIsExporting(true);

		try {
			await graphRef.current.exportSvg();
		} finally {
			setIsExporting(false);
		}
	}

	/**
	 * Exports the visible subgraph as a JSON file.
	 */
	function handleExportJson() {
		if (!graphRef.current) return;

		graphRef.current.exportJson();
	}

	useKeyboardShortcuts({
		onFocusSearch: handleFocusSearch,
		onFitView: handleFitView,
		onClearSelection: handleClearSelection,
		onEscape: () => setSearchQuery(""),
	});

	if (isLoading) {
		return (
			<div className={`${styles["app"]} ${styles["loading"]}`}>
				<p>Loading...</p>
			</div>
		);
	}

	if (!data) {
		return <DataLoader onDataLoaded={setData} />;
	}

	return (
		<div className={styles["app"]}>
			<div className={styles["header"]}>
				<Toolbar
					ref={toolbarRef}
					searchQuery={searchQuery}
					activeFilters={activeFilters}
					advancedFilters={advancedFilters}
					nodeCount={nodeCount}
					visibleCount={visibleCount}
					cycleCount={data.analysis.cycles.length}
					maxDepth={data.analysis.statistics.max_depth}
					maxFanIn={data.analysis.statistics.max_fan_in}
					maxFanOut={data.analysis.statistics.max_fan_out}
					isExporting={isExporting}
					highlightCycles={highlightCycles}
					groupByFolder={groupByFolder}
					onSearchChange={setSearchQuery}
					onFiltersChange={setActiveFilters}
					onAdvancedFiltersChange={setAdvancedFilters}
					onExportPng={handleExportPng}
					onExportSvg={handleExportSvg}
					onExportJson={handleExportJson}
					onFitView={handleFitView}
					onToggleCycles={setHighlightCycles}
					onToggleGroupByFolder={setGroupByFolder}
				/>
			</div>

			<div className={styles["body"]}>
				<main className={styles["main-content"]}>
					<Graph
						ref={graphRef}
						data={data}
						searchQuery={searchQuery}
						activeFilters={activeFilters}
						advancedFilters={advancedFilters}
						pathSource={pathSource}
						pathTarget={pathTarget}
						highlightCycles={highlightCycles}
						groupByFolder={groupByFolder}
						onNodeSelect={handleNodeSelect}
						onEdgeSelect={handleEdgeSelect}
						onClearSelection={handleClearSelection}
					/>

					<Legend />
				</main>

				<Sidebar selectedNode={selectedNode} selectedEdge={selectedEdge} statistics={data.analysis.statistics} edges={data.edges} onFocusNode={handleFocusNode} />
			</div>
		</div>
	);
}

export default App;
