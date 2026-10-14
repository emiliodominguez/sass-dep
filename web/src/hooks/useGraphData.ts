import { useEffect, useState } from "react";

import { describeLoadError, normalizeOutput } from "../utils/normalizeOutput";
import type { SassDepOutput } from "../types/sass-dep";

/** Return type for the useGraphData hook. */
interface UseGraphDataReturn {
	/** The loaded, normalized graph data, or null if not yet loaded. */
	data: SassDepOutput | null;
	/** Whether data is currently being fetched from the API. */
	isLoading: boolean;
	/** Error message if loading or validation failed, or null. */
	error: string | null;
	/**
	 * Validates and normalizes a parsed JSON value and, if valid, makes it the current data.
	 * On failure, sets `error` instead. Used for file uploads.
	 */
	loadData: (raw: unknown) => void;
}

/**
 * Hook for loading graph data from the server API or a file upload.
 * When served by sass-dep's embedded server, it automatically fetches /api/data.
 * Otherwise, it waits for a file upload. All data passes through `normalizeOutput`.
 * @returns Object with data, loading state, error, and loadData function
 */
export function useGraphData(): UseGraphDataReturn {
	const [data, setData] = useState<SassDepOutput | null>(null);
	const [isLoading, setIsLoading] = useState(true);
	const [error, setError] = useState<string | null>(null);

	// Try to fetch from API on mount (for --web mode)
	useEffect(() => {
		let cancelled = false;

		/**
		 * Fetches graph data from the /api/data endpoint and normalizes it.
		 */
		async function fetchFromApi(): Promise<void> {
			let response: Response;
			try {
				response = await fetch("/api/data");
			} catch {
				// Network error or no server - this is fine, the user can upload a file
				console.log("No API available, waiting for file upload");
				if (!cancelled) setIsLoading(false);
				return;
			}

			try {
				if (!response.ok) {
					// No API available (standalone mode, or the dev proxy has no backend)
					console.log(`No API available (HTTP ${response.status}), waiting for file upload`);
					return;
				}

				const contentType = response.headers.get("content-type") ?? "";
				if (!contentType.includes("json")) {
					// Static hosting serving index.html for unknown routes - treat as standalone mode
					return;
				}

				const normalized = normalizeOutput(await response.json());
				if (!cancelled) {
					setData(normalized);
					setError(null);
				}
			} catch (e) {
				if (!cancelled) setError(describeLoadError(e, "Failed to load data from /api/data."));
			} finally {
				if (!cancelled) setIsLoading(false);
			}
		}

		fetchFromApi();

		return () => {
			cancelled = true;
		};
	}, []);

	/**
	 * Validates and normalizes uploaded JSON, storing either the data or an error message.
	 * @param raw - Parsed JSON value of unknown shape
	 */
	function loadData(raw: unknown): void {
		try {
			setData(normalizeOutput(raw));
			setError(null);
		} catch (e) {
			setError(describeLoadError(e, "The file is not valid sass-dep output."));
		}
	}

	return { data, isLoading, error, loadData };
}
