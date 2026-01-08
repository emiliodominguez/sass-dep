import { useCallback, useState } from "react";
import type { SassDepOutput } from "../../types/sass-dep";
import "./DataLoader.css";

interface DataLoaderProps {
	onDataLoaded: (data: SassDepOutput) => void;
}

export function DataLoader({ onDataLoaded }: DataLoaderProps) {
	const [isDragging, setIsDragging] = useState(false);
	const [isLoading, setIsLoading] = useState(false);
	const [error, setError] = useState<string | null>(null);

	const validateAndLoad = useCallback(
		(json: unknown) => {
			if (typeof json !== "object" || json === null) {
				throw new Error("Invalid JSON structure");
			}

			const data = json as SassDepOutput;

			// Basic validation
			if (!data.version || !data.nodes || !data.edges || !data.analysis) {
				throw new Error("Invalid sass-dep output format. Missing required fields.");
			}

			if (data.version !== "1.0.0") {
				console.warn(`Schema version ${data.version} may not be fully compatible`);
			}

			onDataLoaded(data);
		},
		[onDataLoaded],
	);

	const handleFileRead = useCallback(
		async (file: File) => {
			setIsLoading(true);
			setError(null);

			try {
				const text = await file.text();
				const json = JSON.parse(text);
				validateAndLoad(json);
			} catch (e) {
				setError(e instanceof Error ? e.message : "Failed to parse JSON file");
			} finally {
				setIsLoading(false);
			}
		},
		[validateAndLoad],
	);

	const handleDrop = useCallback(
		(e: React.DragEvent) => {
			e.preventDefault();
			setIsDragging(false);

			const file = e.dataTransfer.files[0];
			if (file && file.name.endsWith(".json")) {
				handleFileRead(file);
			} else {
				setError("Please drop a JSON file");
			}
		},
		[handleFileRead],
	);

	const handleDragOver = useCallback((e: React.DragEvent) => {
		e.preventDefault();
		setIsDragging(true);
	}, []);

	const handleDragLeave = useCallback((e: React.DragEvent) => {
		e.preventDefault();
		setIsDragging(false);
	}, []);

	const handleFileSelect = useCallback(
		(e: React.ChangeEvent<HTMLInputElement>) => {
			const file = e.target.files?.[0];
			if (file) {
				handleFileRead(file);
			}
		},
		[handleFileRead],
	);

	return (
		<div className="data-loader">
			<div className="loader-content">
				<h1>sass-dep Visualizer</h1>
				<p className="description">Visualize your SCSS dependency graph interactively</p>

				<div className={`drop-zone ${isDragging ? "dragging" : ""}`} onDrop={handleDrop} onDragOver={handleDragOver} onDragLeave={handleDragLeave}>
					{isLoading ? (
						<p>Loading...</p>
					) : (
						<>
							<p className="drop-text">Drop your sass-dep JSON file here</p>
							<p className="or">or</p>
							<label className="file-input-label">
								<input type="file" accept=".json" onChange={handleFileSelect} className="file-input" />
								Choose File
							</label>
						</>
					)}
				</div>

				{error && <p className="error">{error}</p>}

				<div className="instructions">
					<h3>How to generate the JSON file:</h3>
					<code>sass-dep analyze --output deps.json main.scss</code>
					<p className="tip">
						Or use <code>sass-dep analyze --web main.scss</code> to skip this step!
					</p>
				</div>
			</div>
		</div>
	);
}
