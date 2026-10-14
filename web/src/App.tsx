import { useRef, useState } from "react";

import { useGraphData } from "./hooks/useGraphData";
import { useGraphIndex } from "./hooks/useGraphIndex";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { usePathHighlight } from "./hooks/usePathHighlight";
import { useVisibleNodeIds } from "./hooks/useVisibleNodeIds";
import { Graph, type GraphHandle } from "./components/Graph/Graph";
import { Sidebar } from "./components/Sidebar/Sidebar";
import { DataLoader } from "./components/DataLoader/DataLoader";
import { Legend } from "./components/Legend/Legend";
import { Toolbar } from "./components/Toolbar/Toolbar";
import { ALL_FILTER_FLAGS, type AdvancedFilters, DEFAULT_ADVANCED_FILTERS, type FilterFlag, type ToolbarHandle } from "./components/Toolbar/constants";
import type { OutputEdge, OutputNode, SassDepOutput } from "./types/sass-dep";
import styles from "./App.module.scss";

/** Props for the Workspace component. */
interface WorkspaceProps {
	/** Normalized sass-dep output */
	data: SassDepOutput;
}

/**
 * Main workspace for a loaded dataset: toolbar, graph and sidebar.
 * All per-dataset state lives here, so loading a new dataset starts from a clean slate.
 * @param props - Component props
 * @returns Workspace layout
 */
function Workspace({ data }: WorkspaceProps) {
	const [selectedNode, setSelectedNode] = useState<{ id: string; node: OutputNode } | null>(null);
	const [selectedEdge, setSelectedEdge] = useState<OutputEdge | null>(null);
	const [searchQuery, setSearchQuery] = useState("");
	const [activeFilters, setActiveFilters] = useState<FilterFlag[]>(ALL_FILTER_FLAGS);
	const [advancedFilters, setAdvancedFilters] = useState<AdvancedFilters>(DEFAULT_ADVANCED_FILTERS);
	const [pathSource, setPathSource] = useState<string | null>(null);
	const [pathTarget, setPathTarget] = useState<string | null>(null);
	const [highlightCycles, setHighlightCycles] = useState(false);
	const [highlightImports, setHighlightImports] = useState(false);
	const [groupByFolder, setGroupByFolder] = useState(false);
	const [isExporting, setIsExporting] = useState(false);
	const graphRef = useRef<GraphHandle>(null);
	const toolbarRef = useRef<ToolbarHandle>(null);

	// Built once per dataset and shared by the graph, sidebar and path search.
	const index = useGraphIndex(data);
	// Single source of truth for visibility (toolbar count + graph).
	const visibleIds = useVisibleNodeIds(data, searchQuery, activeFilters, advancedFilters);
	const pathHighlight = usePathHighlight(pathSource, pathTarget, index);
	const nodeCount = data.analysis.statistics.total_files || Object.keys(data.nodes).length;

	/**
	 * Handles node selection, supporting shift-click for path highlighting.
	 * @param nodeId - The selected node's ID
	 * @param node - The selected node data
	 * @param isShiftClick - Whether shift key was held during click
	 */
	function handleNodeSelect(nodeId: string, node: OutputNode, isShiftClick: boolean) {
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
	 * Clears the path endpoints.
	 */
	function handleClearPath() {
		setPathSource(null);
		setPathTarget(null);
	}

	/**
	 * Clears the current selection (node, edge, path and focus highlight).
	 */
	function handleClearSelection() {
		setSelectedNode(null);
		setSelectedEdge(null);
		handleClearPath();
		graphRef.current?.clearFocus();
	}

	/**
	 * Clears the search query.
	 */
	function handleClearSearch() {
		setSearchQuery("");
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
	 * Runs an image export while showing the exporting state in the toolbar.
	 * @param format - Image format to export
	 */
	async function runImageExport(format: "png" | "svg") {
		const graph = graphRef.current;
		if (!graph) return;

		setIsExporting(true);
		try {
			await (format === "png" ? graph.exportPng() : graph.exportSvg());
		} finally {
			setIsExporting(false);
		}
	}

	/**
	 * Exports the current graph view as a PNG image.
	 */
	async function handleExportPng() {
		await runImageExport("png");
	}

	/**
	 * Exports the current graph view as an SVG file.
	 */
	async function handleExportSvg() {
		await runImageExport("svg");
	}

	/**
	 * Exports the visible subgraph as a JSON file.
	 */
	function handleExportJson() {
		graphRef.current?.exportJson();
	}

	useKeyboardShortcuts({
		onFocusSearch: handleFocusSearch,
		onFitView: handleFitView,
		onClearSelection: handleClearSelection,
		onEscape: handleClearSearch,
	});

	return (
		<div className={styles["app"]}>
			<div className={styles["header"]}>
				<Toolbar
					ref={toolbarRef}
					searchQuery={searchQuery}
					activeFilters={activeFilters}
					advancedFilters={advancedFilters}
					nodeCount={nodeCount}
					visibleCount={visibleIds.size}
					cycleCount={data.analysis.cycles.length}
					importerCount={index.importerIds.size}
					maxDepth={data.analysis.statistics.max_depth}
					maxFanIn={data.analysis.statistics.max_fan_in}
					maxFanOut={data.analysis.statistics.max_fan_out}
					isExporting={isExporting}
					highlightCycles={highlightCycles}
					highlightImports={highlightImports}
					groupByFolder={groupByFolder}
					onSearchChange={setSearchQuery}
					onFiltersChange={setActiveFilters}
					onAdvancedFiltersChange={setAdvancedFilters}
					onExportPng={handleExportPng}
					onExportSvg={handleExportSvg}
					onExportJson={handleExportJson}
					onFitView={handleFitView}
					onToggleCycles={setHighlightCycles}
					onToggleImports={setHighlightImports}
					onToggleGroupByFolder={setGroupByFolder}
				/>
			</div>

			<div className={styles["body"]}>
				<main className={styles["main-content"]}>
					<Graph
						ref={graphRef}
						data={data}
						index={index}
						visibleIds={visibleIds}
						pathSource={pathSource}
						pathTarget={pathTarget}
						pathHighlight={pathHighlight}
						highlightCycles={highlightCycles}
						highlightImports={highlightImports}
						groupByFolder={groupByFolder}
						onNodeSelect={handleNodeSelect}
						onEdgeSelect={handleEdgeSelect}
						onClearSelection={handleClearSelection}
					/>

					<Legend />
				</main>

				<Sidebar
					data={data}
					index={index}
					selectedNode={selectedNode}
					selectedEdge={selectedEdge}
					pathSource={pathSource}
					pathTarget={pathTarget}
					pathHighlight={pathHighlight}
					onFocusNode={handleFocusNode}
					onClearPath={handleClearPath}
				/>
			</div>
		</div>
	);
}

/**
 * Main application component.
 * @returns The workspace once data is loaded, otherwise the data loader
 */
function App() {
	const { data, isLoading, error, loadData } = useGraphData();

	if (isLoading) {
		return (
			<div className={`${styles["app"]} ${styles["loading"]}`}>
				<p>Loading...</p>
			</div>
		);
	}

	if (!data) {
		return <DataLoader onLoad={loadData} error={error} />;
	}

	return <Workspace data={data} />;
}

export default App;
