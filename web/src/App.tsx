import { useState, useCallback, useMemo, useRef } from "react";
import { useGraphData } from "./hooks/useGraphData";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { Graph, type GraphHandle } from "./components/Graph/Graph";
import { Sidebar } from "./components/Sidebar/Sidebar";
import { DataLoader } from "./components/DataLoader/DataLoader";
import { Legend } from "./components/Legend/Legend";
import { Toolbar, ALL_FILTER_FLAGS, type ToolbarHandle } from "./components/Toolbar/Toolbar";
import type { OutputNode, OutputEdge } from "./types/sass-dep";
import "./App.css";

function App() {
	const { data, isLoading, setData } = useGraphData();
	const [selectedNode, setSelectedNode] = useState<{ id: string; node: OutputNode } | null>(null);
	const [selectedEdge, setSelectedEdge] = useState<OutputEdge | null>(null);
	const [searchQuery, setSearchQuery] = useState("");
	const [activeFilters, setActiveFilters] = useState<string[]>(ALL_FILTER_FLAGS);
	const graphRef = useRef<GraphHandle>(null);
	const toolbarRef = useRef<ToolbarHandle>(null);

	// Path highlighting state
	const [pathSource, setPathSource] = useState<string | null>(null);
	const [pathTarget, setPathTarget] = useState<string | null>(null);

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

			return matchesSearch && matchesFilters;
		}).length;

		return { nodeCount, visibleCount };
	}, [data, searchQuery, activeFilters]);

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
					nodeCount={nodeCount}
					visibleCount={visibleCount}
					isExporting={isExporting}
					onSearchChange={setSearchQuery}
					onFiltersChange={setActiveFilters}
					onExportPng={handleExportPng}
					onFitView={handleFitView}
				/>
			</div>
			<div className="app-body">
				<main className="main-content">
					<Graph
						ref={graphRef}
						data={data}
						searchQuery={searchQuery}
						activeFilters={activeFilters}
						pathSource={pathSource}
						pathTarget={pathTarget}
						onNodeSelect={handleNodeSelect}
						onEdgeSelect={handleEdgeSelect}
						onClearSelection={handleClearSelection}
					/>
					<Legend />
				</main>
				<Sidebar selectedNode={selectedNode} selectedEdge={selectedEdge} statistics={data.analysis.statistics} onFocusNode={handleFocusNode} />
			</div>
		</div>
	);
}

export default App;
