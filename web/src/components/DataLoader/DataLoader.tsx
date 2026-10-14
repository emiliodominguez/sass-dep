import { useState } from "react";

import styles from "./DataLoader.module.scss";

/** Props for the DataLoader component. */
interface DataLoaderProps {
	/** Receives the parsed JSON; validation and normalization happen in the caller (useGraphData). */
	onLoad: (raw: unknown) => void;
	/** Validation or loading error to display, or null */
	error: string | null;
}

/**
 * Data loader component for importing sass-dep JSON files.
 * @param props - Component props with the load callback and current error
 * @returns A drop zone UI for file upload
 */
export function DataLoader({ onLoad, error }: DataLoaderProps) {
	const [isDragging, setIsDragging] = useState(false);
	const [isLoading, setIsLoading] = useState(false);
	const [localError, setLocalError] = useState<string | null>(null);

	const dropZoneClasses = [styles["drop-zone"], isDragging && styles["dragging"]].filter(Boolean).join(" ");
	const displayedError = localError ?? error;

	/**
	 * Reads and parses a JSON file, then hands it to `onLoad` for validation.
	 * @param file - The file to read
	 */
	async function handleFileRead(file: File) {
		setIsLoading(true);
		setLocalError(null);

		try {
			const text = await file.text();
			let json: unknown;
			try {
				json = JSON.parse(text);
			} catch (e) {
				setLocalError(`${file.name} is not valid JSON${e instanceof Error ? `: ${e.message}` : "."}`);
				return;
			}
			onLoad(json);
		} catch (e) {
			setLocalError(e instanceof Error ? e.message : "Failed to read the file.");
		} finally {
			setIsLoading(false);
		}
	}

	/**
	 * Handles file drop events.
	 * @param e - The drag event
	 */
	function handleDrop(e: React.DragEvent) {
		e.preventDefault();
		setIsDragging(false);

		const file = e.dataTransfer.files[0];
		if (file && file.name.endsWith(".json")) {
			handleFileRead(file);
		} else {
			setLocalError("Please drop a JSON file");
		}
	}

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
	function handleFileSelect(e: React.ChangeEvent<HTMLInputElement>) {
		const file = e.target.files?.[0];
		// Reset so selecting the same file again (after fixing it) triggers another change event
		e.target.value = "";
		if (file) {
			handleFileRead(file);
		}
	}

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
								<input type="file" accept=".json,application/json" onChange={handleFileSelect} className={styles["file-input"]} />
								Choose File
							</label>
						</>
					)}
				</div>

				{displayedError && (
					<p className={styles["error"]} role="alert">
						{displayedError}
					</p>
				)}

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
