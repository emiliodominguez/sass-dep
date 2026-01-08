import { useState, useCallback, useMemo, useRef } from "react";
import { useGraphData } from "./hooks/useGraphData";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { Graph, type GraphHandle } from "./components/Graph/Graph";
import { Sidebar } from "./components/Sidebar/Sidebar";
import { DataLoader } from "./components/DataLoader/DataLoader";
import { Legend } from "./components/Legend/Legend";
import { Toolbar, ALL_FILTER_FLAGS, DEFAULT_ADVANCED_FILTERS, type ToolbarHandle, type AdvancedFilters } from "./components/Toolbar/Toolbar";
import type { OutputNode, OutputEdge } from "./types/sass-dep";
import "./App.css";

function App() {
	const { data, isLoading, setData } = useGraphData();
	const [selectedNode, setSelectedNode] = useState<{ id: string; node: OutputNode } | null>(null);
	const [selectedEdge, setSelectedEdge] = useState<OutputEdge | null>(null);
	const [searchQuery, setSearchQuery] = useState("");
	const [activeFilters, setActiveFilters] = useState<string[]>(ALL_FILTER_FLAGS);
	const [advancedFilters, setAdvancedFilters] = useState<AdvancedFilters>(DEFAULT_ADVANCED_FILTERS);
	const graphRef = useRef<GraphHandle>(null);
	const toolbarRef = useRef<ToolbarHandle>(null);

	// Path highlighting state
	const [pathSource, setPathSource] = useState<string | null>(null);
	const [pathTarget, setPathTarget] = useState<string | null>(null);

	// Cycle highlighting state
	const [highlightCycles, setHighlightCycles] = useState(false);

	// Folder grouping state
	const [groupByFolder, setGroupByFolder] = useState(false);

	const handleNodeSelect = useCallback(
		(nodeId: string, node: OutputNode, isShiftClick?: boolean) => {
			if (isShiftClick) {
				// Shift+click for path selection
				if (!pathSource) {
					setPathSource(nodeId);
					setPathTarget(null);
				} else if (pathSource === nodeId) {
					// Clicking same node clears path
					setPathSource(null);
					setPathTarget(null);
				} else {
					setPathTarget(nodeId);
				}
			} else {
				// Regular click clears path mode
				setPathSource(null);
				setPathTarget(null);
			}
			setSelectedNode({ id: nodeId, node });
			setSelectedEdge(null);
		},
		[pathSource],
	);

	const handleEdgeSelect = useCallback((edge: OutputEdge) => {
		setSelectedEdge(edge);
		setSelectedNode(null);
	}, []);

	const handleClearSelection = useCallback(() => {
		setSelectedNode(null);
		setSelectedEdge(null);
		setPathSource(null);
		setPathTarget(null);
	}, []);

	const handleFocusNode = useCallback((nodeId: string) => {
		graphRef.current?.focusNode(nodeId);
	}, []);

	const handleFitView = useCallback(() => {
		graphRef.current?.fitView();
	}, []);

	const handleFocusSearch = useCallback(() => {
		toolbarRef.current?.focusSearch();
	}, []);

	// Keyboard shortcuts
	useKeyboardShortcuts({
		onFocusSearch: handleFocusSearch,
		onFitView: handleFitView,
		onClearSelection: handleClearSelection,
		onEscape: () => setSearchQuery(""),
	});

	const [isExporting, setIsExporting] = useState(false);

	const handleExportPng = useCallback(async () => {
		if (!graphRef.current) return;
		setIsExporting(true);
		try {
			await graphRef.current.exportPng();
		} finally {
			setIsExporting(false);
		}
	}, []);

	const handleExportSvg = useCallback(async () => {
		if (!graphRef.current) return;
		setIsExporting(true);
		try {
			await graphRef.current.exportSvg();
		} finally {
			setIsExporting(false);
		}
	}, []);

	const handleExportJson = useCallback(() => {
		if (!graphRef.current) return;
		graphRef.current.exportJson();
	}, []);

	// Calculate visible node count for toolbar stats
	const { nodeCount, visibleCount } = useMemo(() => {
		if (!data) return { nodeCount: 0, visibleCount: 0 };

		const nodes = Object.entries(data.nodes);
		const nodeCount = nodes.length;

		const visibleCount = nodes.filter(([id, node]) => {
			const matchesSearch = !searchQuery || id.toLowerCase().includes(searchQuery.toLowerCase()) || node.path.toLowerCase().includes(searchQuery.toLowerCase());

			const matchesFilters =
				activeFilters.length === 0 ||
				activeFilters.some((filter) => {
					if (filter === "no_flags") {
						return node.flags.length === 0;
					}
					return node.flags.includes(filter as OutputNode["flags"][number]);
				});

			// Check advanced filters
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

		return { nodeCount, visibleCount };
	}, [data, searchQuery, activeFilters, advancedFilters]);

	// Show loading state
	if (isLoading) {
		return (
			<div className="app loading">
				<p>Loading...</p>
			</div>
		);
	}

	// Show data loader if no data
	if (!data) {
		return <DataLoader onDataLoaded={setData} />;
	}

	// Show graph visualization
	return (
		<div className="app">
			<div className="app-header">
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
			<div className="app-body">
				<main className="main-content">
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
