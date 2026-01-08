import { useState, useEffect, useCallback } from "react";
import type { SassDepOutput } from "../types/sass-dep";

interface UseGraphDataReturn {
	data: SassDepOutput | null;
	isLoading: boolean;
	error: string | null;
	setData: (data: SassDepOutput) => void;
}

/**
 * Hook for loading graph data from the server API or file upload.
 *
 * When served by sass-dep's embedded server, it automatically fetches
 * from /api/data. Otherwise, it waits for file upload.
 */
export function useGraphData(): UseGraphDataReturn {
	const [data, setData] = useState<SassDepOutput | null>(null);
	const [isLoading, setIsLoading] = useState(true);
	const [error, setError] = useState<string | null>(null);

	// Try to fetch from API on mount (for --web mode)
	useEffect(() => {
		const fetchFromApi = async () => {
			try {
				const response = await fetch("/api/data");

				if (response.ok) {
					const json = await response.json();
					setData(json);
					setError(null);
				} else if (response.status === 404) {
					// No API available (standalone mode)
					setError(null);
				} else {
					throw new Error(`HTTP ${response.status}`);
				}
			} catch (e) {
				// Network error or no server - this is fine, user can upload file
				console.log("No API available, waiting for file upload");
				setError(null);
			} finally {
				setIsLoading(false);
			}
		};

		fetchFromApi();
	}, []);

	const handleSetData = useCallback((newData: SassDepOutput) => {
		setData(newData);
		setError(null);
	}, []);

	return {
		data,
		isLoading,
		error,
		setData: handleSetData,
	};
}
