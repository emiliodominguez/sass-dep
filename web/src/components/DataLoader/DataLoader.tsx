import { useCallback, useState } from "react";

import type { SassDepOutput } from "../../types/sass-dep";
import styles from "./DataLoader.module.scss";

interface DataLoaderProps {
	onDataLoaded: (data: SassDepOutput) => void;
}

/**
 * Data loader component for importing sass-dep JSON files.
 * @param props - Component props with callback for loaded data
 * @returns A drop zone UI for file upload
 */
export function DataLoader({ onDataLoaded }: DataLoaderProps) {
	const [isDragging, setIsDragging] = useState(false);
	const [isLoading, setIsLoading] = useState(false);
	const [error, setError] = useState<string | null>(null);

	const dropZoneClasses = [styles["drop-zone"], isDragging && styles["dragging"]].filter(Boolean).join(" ");

	/**
	 * Validates JSON data and passes it to the onDataLoaded callback.
	 * @param json - The parsed JSON data to validate
	 * @throws Error if the JSON structure is invalid
	 */
	const validateAndLoad = useCallback(
		(json: unknown) => {
			if (typeof json !== "object" || json === null) {
				throw new Error("Invalid JSON structure");
			}

			const data = json as SassDepOutput;

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

	/**
	 * Reads and parses a JSON file.
	 * @param file - The file to read
	 */
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

	/**
	 * Handles file drop events.
	 * @param e - The drag event
	 */
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

	/**
	 * Handles drag over events to show drop zone state.
	 * @param e - The drag event
	 */
	function handleDragOver(e: React.DragEvent) {
		e.preventDefault();
		setIsDragging(true);
	}

	/**
	 * Handles drag leave events to reset drop zone state.
	 * @param e - The drag event
	 */
	function handleDragLeave(e: React.DragEvent) {
		e.preventDefault();
		setIsDragging(false);
	}

	/**
	 * Handles file input selection.
	 * @param e - The change event from file input
	 */
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
		<div className={styles["data-loader"]}>
			<div className={styles["content"]}>
				<h1>sass-dep Visualizer</h1>
				<p className={styles["description"]}>Visualize your SCSS dependency graph interactively</p>

				<div className={dropZoneClasses} onDrop={handleDrop} onDragOver={handleDragOver} onDragLeave={handleDragLeave}>
					{isLoading ? (
						<p>Loading...</p>
					) : (
						<>
							<p className={styles["drop-text"]}>Drop your sass-dep JSON file here</p>
							<p className={styles["or"]}>or</p>
							<label className={styles["file-input-label"]}>
								<input type="file" accept=".json" onChange={handleFileSelect} className={styles["file-input"]} />
								Choose File
							</label>
						</>
					)}
				</div>

				{error && <p className={styles["error"]}>{error}</p>}

				<div className={styles["instructions"]}>
					<h3>How to generate the JSON file:</h3>
					<code>sass-dep analyze --output deps.json main.scss</code>
					<p className={styles["tip"]}>
						Or use <code>sass-dep analyze --web main.scss</code> to skip this step!
					</p>
				</div>
			</div>
		</div>
	);
}
